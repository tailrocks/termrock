use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use termrock_core::id::{PartRef, WidgetId};
use termrock_runtime::ctx::{RenderCtx, fill};
use termrock_runtime::measure::terminal_extent;
use termrock_runtime::tooltip::Tip;
use termrock_text::cells::width;
use termrock_theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconLook {
    #[default]
    Close,

    Title,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconButton {
    pub id: WidgetId,

    pub glyph: Option<String>,
    pub look: IconLook,

    pub pad: u16,

    pub danger_hover: bool,

    pub tip: Option<Tip>,

    pub part: Option<(WidgetId, PartRef)>,
}

impl IconButton {
    pub const DEFAULT_PAD: u16 = 1;

    pub fn close(id: WidgetId) -> Self {
        Self {
            id,
            glyph: None,
            look: IconLook::Close,
            pad: Self::DEFAULT_PAD,
            danger_hover: false,
            tip: None,
            part: None,
        }
    }

    pub fn new(id: WidgetId, glyph: &str) -> Self {
        Self {
            glyph: Some(glyph.to_owned()),
            look: IconLook::Title,
            ..Self::close(id)
        }
    }

    pub fn with_id(&self, id: WidgetId) -> Self {
        Self {
            id,
            part: None,
            ..self.clone()
        }
    }

    pub fn part_of(&self, owner: WidgetId, part: PartRef) -> Self {
        Self {
            id: owner.part(part),
            part: Some((owner, part)),
            ..self.clone()
        }
    }

    pub fn with_glyph(&self, glyph: &str) -> Self {
        Self {
            glyph: Some(glyph.to_owned()),
            ..self.clone()
        }
    }

    pub fn pad(mut self, cells: u16) -> Self {
        self.pad = cells;
        self
    }

    pub fn plate(self, cells: u16) -> Self {
        self.pad(cells.saturating_sub(1) / 2)
    }

    pub fn danger_hover(mut self, on: bool) -> Self {
        self.danger_hover = on;
        self
    }

    pub fn tip(mut self, tip: Tip) -> Self {
        self.tip = Some(tip);
        self
    }

    pub fn glyph<'a>(&'a self, t: &Theme) -> &'a str {
        self.glyph.as_deref().unwrap_or(t.glyphs.close)
    }

    /// Glyph plus padding on both sides, saturating at `u16::MAX`.
    pub fn width(&self, t: &Theme) -> u16 {
        terminal_extent(self.intrinsic_width(t))
    }

    pub(crate) fn intrinsic_width(&self, t: &Theme) -> usize {
        width(self.glyph(t)).saturating_add(2 * usize::from(self.pad))
    }

    pub fn style(&self, ctx: &RenderCtx, host: Style) -> Style {
        let t = ctx.theme;
        let mut s = ctx.state(self.id);
        s.focused = false;
        match self.look {
            IconLook::Close => t.close_plate(s, host, self.danger_hover),
            IconLook::Title => {
                let bg = host.bg.unwrap_or(t.canvas);
                t.danger_plate(s, ctx.title_style(self.id, bg), self.danger_hover)
            }
        }
    }

    pub fn render(
        &self,
        x: u16,
        y: u16,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        host: Style,
        default_tip: Option<Tip>,
    ) -> Rect {
        let plate = Rect::new(x, y, self.width(ctx.theme), 1).intersection(*buf.area());
        fill(buf, plate, host);
        self.render_in(
            plate,
            x.saturating_add(self.pad),
            buf,
            ctx,
            host,
            default_tip,
        )
    }

    pub fn render_in(
        &self,
        plate: Rect,
        glyph_x: u16,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        host: Style,
        default_tip: Option<Tip>,
    ) -> Rect {
        let plate = plate.intersection(*buf.area());
        if plate.is_empty() {
            return Rect::ZERO;
        }
        let st = self.style(ctx, host);
        let s = ctx.state(self.id);
        if s.hovered || s.pressed {
            for x in plate.left()..plate.right() {
                if let Some(cell) = buf.cell_mut((x, plate.y)) {
                    cell.set_style(Style::reset().patch(st));
                }
            }
        }
        if let Some(col) = glyph_x.checked_sub(plate.x) {
            termrock_text::cells::paint_clipped(
                buf,
                plate,
                usize::from(col),
                self.glyph(ctx.theme),
                st,
            );
        }
        match self.part {
            Some((owner, part)) => {
                ctx.register_part(owner, part, plate);
            }
            None => ctx.clickable(self.id, plate),
        }
        if let Some(tip) = self.tip.clone().or(default_tip) {
            termrock_runtime::tooltip::show(ctx, self.id, plate, &tip, None);
        }
        plate
    }
}

impl termrock_runtime::specimen::Specimen for IconButton {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: termrock_runtime::specimen::SpecimenState) -> bool {
        use termrock_runtime::specimen::SpecimenState::*;
        matches!(state, Rest | Hover | Pressed)
    }

    fn specimen(
        &self,
        _state: termrock_runtime::specimen::SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: ratatui::style::Color,
    ) {
        if area.is_empty() {
            return;
        }
        let host = Style::default().fg(ctx.theme.text_secondary).bg(bg);
        self.render(area.x, area.y, buf, ctx, host, Some(Tip::default()));
    }
}
