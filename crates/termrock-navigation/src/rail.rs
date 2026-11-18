use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use termrock_core::id::{Part, PartRef, WidgetId};
use termrock_runtime::ctx::RenderCtx;
use termrock_runtime::tooltip::Tip;
use termrock_text::cells::{paint_clipped, width};
use termrock_theme::Kind;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RailAction {
    Expand,

    Collapse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarRail {
    pub id: WidgetId,

    pub label: String,

    pub collapsed_rail: bool,
}

impl SidebarRail {
    pub const WIDTH: u16 = 2;

    pub fn new(id: WidgetId, label: &str) -> Self {
        Self {
            id,
            label: label.to_owned(),
            collapsed_rail: true,
        }
    }

    pub fn collapsed_rail(mut self, on: bool) -> Self {
        self.collapsed_rail = on;
        self
    }

    pub fn collapse_id(&self) -> WidgetId {
        self.id.part(PartRef::of(Part::ICON))
    }

    pub fn width(&self) -> u16 {
        if self.collapsed_rail { Self::WIDTH } else { 0 }
    }

    pub fn on_click(&self, id: WidgetId) -> Option<RailAction> {
        if id == self.id && self.collapsed_rail {
            Some(RailAction::Expand)
        } else if id == self.collapse_id() {
            Some(RailAction::Collapse)
        } else {
            None
        }
    }

    /// One entry per vertical row: the cluster itself when it is one cell
    /// wide, `None` for a cluster wider than the column. Zero-width clusters
    /// take no row.
    fn rows(&self) -> impl Iterator<Item = Option<&str>> {
        self.label.graphemes(true).filter_map(|g| match width(g) {
            0 => None,
            1 => Some(Some(g)),
            _ => Some(None),
        })
    }

    pub fn render_rail(&self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        let area = area.intersection(*buf.area());
        if !self.collapsed_rail || area.is_empty() {
            return;
        }
        let t = ctx.theme;
        let col = Rect::new(area.x, area.y, 1, area.height);
        let st = t
            .interactive(Kind::Title, ctx.state(self.id), t.plane_of(bg))
            .style;
        termrock_runtime::ctx::fill(buf, col, st);
        paint_clipped(buf, col, 0, t.glyphs.overflow_right, st);
        let placeholder = if width(t.glyphs.ellipsis) == 1 {
            t.glyphs.ellipsis
        } else {
            "…"
        };
        for (i, cluster) in self.rows().enumerate() {
            let y = col.y.saturating_add(2 + i as u16);
            if y >= col.bottom() {
                break;
            }
            let cell = Rect::new(col.x, y, 1, 1);
            paint_clipped(buf, cell, 0, cluster.unwrap_or(placeholder), st);
        }
        ctx.clickable(self.id, col);
        if !self.label.is_empty() {
            termrock_runtime::tooltip::show(ctx, self.id, col, &Tip::new(&self.label), None);
        }
    }

    pub fn render_collapse(&self, panel: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        let panel = panel.intersection(*buf.area());
        if panel.width < 12 || panel.height < 2 {
            return;
        }
        let t = ctx.theme;
        let id = self.collapse_id();
        let r = Rect::new(panel.right() - 5, panel.y, 3, 1);
        let host = t.border(false).bg(bg);
        let st = t
            .interactive(Kind::Title, ctx.state(id), t.plane_of(bg))
            .style;
        paint_clipped(buf, r, 0, " ", host);
        paint_clipped(buf, r, 2, " ", host);
        paint_clipped(buf, r, 1, t.glyphs.overflow_left, st);
        ctx.register_part(self.id, PartRef::of(Part::ICON), r);
    }
}

impl termrock_runtime::specimen::Specimen for SidebarRail {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: termrock_runtime::specimen::SpecimenState) -> bool {
        use termrock_runtime::specimen::SpecimenState::*;
        self.collapsed_rail && matches!(state, Rest | Hover | Pressed)
    }

    fn specimen_height(&self, _state: termrock_runtime::specimen::SpecimenState) -> u16 {
        u16::try_from(2 + self.rows().count()).unwrap_or(u16::MAX)
    }

    fn specimen(
        &self,
        _state: termrock_runtime::specimen::SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        termrock_runtime::ctx::fill(buf, area, ratatui::style::Style::new().bg(bg));
        self.render_rail(area, buf, ctx, bg);
    }
}

impl termrock_core::pointer::MouseTarget for SidebarRail {
    type Action = RailAction;

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn pointer(
        &mut self,
        ev: &termrock_core::pointer::PointerEvent,
    ) -> termrock_core::response::Response<RailAction> {
        use termrock_core::response::Response;
        if !ev.is(termrock_core::intent::Phase::Click) {
            return Response::ignored();
        }
        match self.on_click(ev.id) {
            Some(a) => Response::emit(a),
            None => Response::ignored(),
        }
    }
}
