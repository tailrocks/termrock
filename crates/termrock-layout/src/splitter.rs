use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Color;

use crate::core::event::Outcome;
use crate::core::id::WidgetId;
use crate::theme::Kind;
use crate::ui::ctx::{RenderCtx, VisualState};
use crate::ui::layout::{Split, SplitDir};
use crate::ui::measure::terminal_extent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SeamDoubleClick {
    #[default]
    Reset,

    Ignore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Splitter {
    pub id: WidgetId,
    pub dir: SplitDir,

    pub area: Rect,

    pub hit_pad: u16,

    pub double_click: SeamDoubleClick,

    grab: Option<i32>,
}

impl Splitter {
    pub const fn new(id: WidgetId, dir: SplitDir) -> Self {
        Self {
            id,
            dir,
            area: Rect::ZERO,
            hit_pad: 1,
            double_click: SeamDoubleClick::Reset,
            grab: None,
        }
    }

    pub const fn hit_pad(mut self, cells: u16) -> Self {
        self.hit_pad = cells;
        self
    }

    pub const fn double_click(mut self, policy: SeamDoubleClick) -> Self {
        self.double_click = policy;
        self
    }

    pub fn hit_area(&self, clip: Rect) -> Rect {
        if self.area.is_empty() {
            return Rect::ZERO;
        }
        let p = self.hit_pad;
        let r = match self.dir {
            SplitDir::Horizontal => {
                let x = self.area.x.saturating_sub(p);
                let end = usize::from(self.area.right()) + usize::from(p);
                Rect::new(
                    x,
                    self.area.y,
                    terminal_extent(end - usize::from(x)),
                    self.area.height,
                )
            }
            SplitDir::Vertical => {
                let y = self.area.y.saturating_sub(p);
                let end = usize::from(self.area.bottom()) + usize::from(p);
                Rect::new(
                    self.area.x,
                    y,
                    self.area.width,
                    terminal_extent(end - usize::from(y)),
                )
            }
        };
        r.intersection(clip)
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        self.render_held(area, buf, ctx, bg, false);
    }

    pub fn render_held(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
        held: bool,
    ) {
        let area = area.intersection(*buf.area());
        self.area = area;

        let pressed = held || ctx.interaction.pressed == Some(self.id);
        if !pressed {
            self.grab = None;
        }
        if area.is_empty() {
            return;
        }
        let t = ctx.theme;
        let hovered = ctx.interaction.hovered(self.id);
        let s = VisualState {
            hovered,
            pressed,
            ..Default::default()
        };
        let style = t.interactive(Kind::Seam, s, t.plane_of(bg)).style.bg(bg);
        let glyph = match (self.dir, hovered || pressed) {
            (SplitDir::Horizontal, false) => t.glyphs.divider,
            (SplitDir::Horizontal, true) => t.glyphs.scroll_thumb,
            (SplitDir::Vertical, false) => t.glyphs.h_divider,
            (SplitDir::Vertical, true) => t.glyphs.scroll_h_thumb,
        };
        for pos in area.positions() {
            buf.set_string(pos.x, pos.y, glyph, style);
        }

        let hit = self.hit_area(*buf.area());
        ctx.clickable_under(self.id, hit);
        ctx.clickable(self.id, area);
        ctx.pointer_shape(
            self.id,
            match self.dir {
                SplitDir::Horizontal => crate::core::shape::PointerShape::ColResize,
                SplitDir::Vertical => crate::core::shape::PointerShape::RowResize,
            },
        );
    }

    fn grab_at(&self, split: &Split, container: Rect, gap: u16, pos: Position) -> i32 {
        let handle = split.handle(self.dir, container, gap);
        let (at, span) = match self.dir {
            SplitDir::Horizontal => (pos.x as i32 - handle.x as i32, handle.width),
            SplitDir::Vertical => (pos.y as i32 - handle.y as i32, handle.height),
        };
        let pad = self.hit_pad as i32;
        at.clamp(-pad, (span as i32 - 1).max(0) + pad)
    }

    pub fn on_press(&mut self, split: &Split, container: Rect, gap: u16, pos: Position) {
        self.grab = Some(self.grab_at(split, container, gap, pos));
    }

    pub fn on_drag(
        &mut self,
        split: &mut Split,
        container: Rect,
        gap: u16,
        pos: Position,
    ) -> Outcome {
        let grab = match self.grab {
            Some(g) => g,
            None => {
                let g = self.grab_at(split, container, gap, pos);
                self.grab = Some(g);
                g
            }
        };
        let shift = |v: u16| (v as i32 - grab).max(0) as u16;
        let target = match self.dir {
            SplitDir::Horizontal => Position::new(shift(pos.x), pos.y),
            SplitDir::Vertical => Position::new(pos.x, shift(pos.y)),
        };
        if split.drag_to(self.dir, container, gap, target) {
            Outcome::Changed
        } else {
            Outcome::Consumed
        }
    }

    /// Drops the grab without touching the split. Returns whether a grab was held.
    pub fn cancel_drag(&mut self) -> bool {
        self.grab.take().is_some()
    }

    pub fn on_click(&mut self, split: &mut Split, count: u8) -> Outcome {
        self.grab = None;
        if count != 2 {
            return Outcome::Consumed;
        }
        match self.double_click {
            SeamDoubleClick::Reset => {
                let before = *split;
                split.reset();
                if *split == before {
                    Outcome::Consumed
                } else {
                    Outcome::Changed
                }
            }
            SeamDoubleClick::Ignore => Outcome::Consumed,
        }
    }
}

impl crate::ui::specimen::Specimen for Splitter {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: crate::ui::specimen::SpecimenState) -> bool {
        use crate::ui::specimen::SpecimenState::*;
        matches!(state, Rest | Hover | Pressed)
    }

    fn specimen_height(&self, _state: crate::ui::specimen::SpecimenState) -> u16 {
        match self.dir {
            SplitDir::Horizontal => 3,
            SplitDir::Vertical => 1,
        }
    }

    fn specimen(
        &self,
        _state: crate::ui::specimen::SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        let strip = match self.dir {
            SplitDir::Horizontal => Rect::new(area.x + 1, area.y, 1, area.height),
            SplitDir::Vertical => Rect::new(area.x, area.y, area.width.min(12), 1),
        };
        let mut s = *self;
        s.render(strip, buf, ctx, bg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitter_cancel_drag_is_idempotent() {
        let mut seam = Splitter::new(WidgetId::of("t.seam"), SplitDir::Horizontal);
        let mut split = Split::new(40, 1, 1);
        let container = Rect::new(0, 0, 40, 5);
        seam.on_press(&split, container, 1, Position::new(20, 2));
        seam.on_drag(&mut split, container, 1, Position::new(25, 2));
        let dragged = split;

        assert!(seam.cancel_drag(), "the held grab is dropped");
        assert!(!seam.cancel_drag(), "a second cancel finds nothing to drop");
        assert_eq!(split, dragged, "cancelling never touches the split ratio");
    }
}
