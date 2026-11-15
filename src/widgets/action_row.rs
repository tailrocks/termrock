use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::core::id::WidgetId;
use crate::theme::Tone;
use crate::ui::ctx::RenderCtx;
use crate::ui::measure::terminal_extent;
use crate::ui::text::{truncate, width};
use crate::widgets::button::{Button, ButtonRow, RowAlign, place_lines};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionRow {
    pub align: RowAlign,

    pub gap: u16,

    pub message: Option<(String, Tone)>,

    pub wrap: bool,
}

impl Default for ActionRow {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionRow {
    pub fn new() -> Self {
        Self {
            align: RowAlign::Start,
            gap: 2,
            message: None,
            wrap: false,
        }
    }

    pub fn align(mut self, align: RowAlign) -> Self {
        self.align = align;
        self
    }

    pub fn gap(mut self, gap: u16) -> Self {
        self.gap = gap;
        self
    }

    pub fn message(mut self, text: &str, tone: Tone) -> Self {
        self.message = Some((text.to_owned(), tone));
        self
    }

    pub fn wrap(mut self, on: bool) -> Self {
        self.wrap = on;
        self
    }

    /// Natural width of the buttons and message, saturating at `u16::MAX`.
    pub fn width(&self, buttons: &[&Button]) -> u16 {
        terminal_extent(self.intrinsic_width(buttons))
    }

    pub(crate) fn intrinsic_width(&self, buttons: &[&Button]) -> usize {
        let gap = usize::from(self.gap);
        let row = buttons
            .iter()
            .map(|b| b.intrinsic_width())
            .fold(0usize, usize::saturating_add)
            .saturating_add(gap.saturating_mul(buttons.len().saturating_sub(1)));
        match &self.message {
            Some((text, _)) => row.saturating_add(gap).saturating_add(width(text)),
            None => row,
        }
    }

    pub fn layout(&self, area: Rect, buttons: &[&Button]) -> Vec<Rect> {
        let widths: Vec<usize> = buttons.iter().map(|b| b.intrinsic_width()).collect();
        let row = Rect::new(area.x, area.y, area.width, area.height.min(1));
        place_lines(row, &widths, self.gap, self.align == RowAlign::End)
    }

    pub fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
        buttons: &mut [&mut Button],
    ) -> Vec<Rect> {
        let rects = {
            let view: Vec<&Button> = buttons.iter().map(|b| &**b).collect();
            self.layout(area, &view)
        };
        for (b, r) in buttons.iter_mut().zip(&rects) {
            b.render(*r, buf, ctx, bg);
        }
        if let Some((text, tone)) = &self.message
            && !area.is_empty()
        {
            let t = ctx.theme;
            let ink = Style::new().fg(t.tone_on(*tone, bg)).bg(bg);
            match self.align {
                RowAlign::End => {
                    let left = rects.first().map_or(area.right(), |r| r.x);
                    let room = left.saturating_sub(area.x.saturating_add(self.gap));
                    if room > 0 {
                        buf.set_string(area.x, area.y, truncate(text, room as usize), ink);
                    }
                }
                _ => {
                    let x = rects
                        .iter()
                        .filter(|r| r.width > 0)
                        .map(|r| r.right().saturating_add(self.gap))
                        .max()
                        .unwrap_or(area.x);
                    let room = area.right().saturating_sub(x);
                    if room > 0 && x < area.right() {
                        let s = truncate(text, room as usize);
                        if width(&s) > 0 {
                            buf.set_string(x, area.y, s, ink);
                        }
                    }
                }
            }
        }
        rects
    }

    pub fn step(
        &self,
        buttons: &[&Button],
        cur: Option<WidgetId>,
        forward: bool,
    ) -> Option<WidgetId> {
        ButtonRow::new()
            .wrap(self.wrap)
            .step(buttons.iter().copied(), cur, forward)
    }
}
