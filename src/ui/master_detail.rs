use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::core::event::{Key, Outcome};
use crate::core::focus::Focus;
use crate::core::id::WidgetId;
use crate::core::intent::Phase;
use crate::core::pointer::PointerEvent;
use crate::ui::ctx::RenderCtx;
use crate::ui::layout::{Split, SplitDir, SplitKeys};
use crate::widgets::keyhint::{Hint, hint};
use crate::widgets::menu::MenuItem;
use crate::widgets::splitter::Splitter;

pub const SPLIT_MIN: u16 = 108;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailSize {
    Share { pct: u16, min: u16, max: u16 },

    MasterShare { pct: u16, min: u16, max: u16 },

    Resizable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Split,

    Master,

    Drawer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MdLayout {
    pub mode: Mode,
    pub master: Rect,
    pub detail: Rect,

    pub summary: Rect,

    pub seam: Rect,
}

#[derive(Debug, Clone)]
pub struct MasterDetail {
    pub master: WidgetId,

    pub detail: WidgetId,

    pub detail_owns: Vec<WidgetId>,

    pub size: DetailSize,

    pub split: Split,

    pub seam: Splitter,

    pub keys: SplitKeys,

    pub gap: u16,

    pub collapse_below: u16,

    pub summary_rows: u16,

    pub esc_returns: bool,
    drawer: bool,
    last: MdLayout,

    container: Rect,
}

impl MasterDetail {
    pub fn new(master: WidgetId, detail: WidgetId) -> Self {
        Self {
            master,
            detail,
            detail_owns: Vec::new(),
            size: DetailSize::Resizable,
            split: Split::new(40, 28, 40),
            seam: Splitter::new(master.sub("seam"), SplitDir::Horizontal),
            keys: SplitKeys::default(),
            gap: 2,
            collapse_below: SPLIT_MIN,
            summary_rows: 0,
            esc_returns: true,
            drawer: false,
            last: MdLayout::default(),
            container: Rect::ZERO,
        }
    }

    pub fn natural_width(&self, master: u16, detail: u16) -> u16 {
        self.panes_width(master, detail).max(self.collapse_below)
    }

    /// Desired extent for the given pane widths. When no representable width
    /// can hold the configured share minima, this saturates at `u16::MAX`.
    pub fn panes_width(&self, master: u16, detail: u16) -> u16 {
        let (pct, min, max, detail_share) = match self.size {
            DetailSize::Resizable => {
                return self.split.natural_extent(master, detail, self.gap);
            }
            DetailSize::Share { pct, min, max } => (pct, min, max, true),
            DetailSize::MasterShare { pct, min, max } => (pct, min, max, false),
        };
        let (master, detail) = if detail_share {
            (master, detail.min(max.max(min)))
        } else {
            (master.min(max.max(min)), detail)
        };
        let fits = |w: u16| {
            share_widths(w, self.gap, pct, min, max, detail_share)
                .is_some_and(|(m, _, d)| m >= master && d >= detail)
        };
        let mut w = master.saturating_add(detail).saturating_add(self.gap);
        while w < u16::MAX && !fits(w) {
            w += 1;
        }
        w
    }

    /// Splits `area` into master and detail rectangles inside it.
    ///
    /// Returns `None` when the area is narrower than `collapse_below`, or when
    /// a `Share`/`MasterShare` size cannot fit: the area is empty or the
    /// configured minimum exceeds the width left after the gap. Callers then use
    /// the collapsed layout, as [`MasterDetail::layout`] does.
    pub fn split_rects(&self, area: Rect) -> Option<(Rect, Rect)> {
        if area.width < self.collapse_below {
            return None;
        }
        let (pct, min, max, detail_share) = match self.size {
            DetailSize::Resizable => return Some(self.split.horizontal(area, self.gap)),
            DetailSize::Share { pct, min, max } => (pct, min, max, true),
            DetailSize::MasterShare { pct, min, max } => (pct, min, max, false),
        };
        if area.is_empty() {
            return None;
        }
        let (mw, gap, dw) = share_widths(area.width, self.gap, pct, min, max, detail_share)?;
        Some((
            Rect::new(area.x, area.y, mw, area.height),
            Rect::new(area.x + mw + gap, area.y, dw, area.height),
        ))
    }

    pub fn collapsed_mode(&self, focus: &crate::ui::ctx::Interaction) -> Mode {
        let open = self.drawer && !focus.focused(self.master);
        let detail_focused = focus.focus.is_some_and(|f| self.in_detail(f)) && !focus.focus_hidden;
        if detail_focused || open {
            Mode::Drawer
        } else {
            Mode::Master
        }
    }

    pub fn collapse_below(mut self, width: u16) -> Self {
        self.collapse_below = width;
        self
    }

    pub fn detail_share(mut self, pct: u16, min: u16, max: u16) -> Self {
        self.size = DetailSize::Share { pct, min, max };
        self
    }

    pub fn master_share(mut self, pct: u16, min: u16, max: u16) -> Self {
        self.size = DetailSize::MasterShare { pct, min, max };
        self
    }

    pub fn resizable(mut self, split: Split) -> Self {
        self.size = DetailSize::Resizable;
        self.split = split;
        self
    }

    pub fn seam(mut self, seam: Splitter) -> Self {
        self.seam = seam;
        self
    }

    pub fn keys(mut self, keys: SplitKeys) -> Self {
        self.keys = keys;
        self
    }

    pub fn gap(mut self, cols: u16) -> Self {
        self.gap = cols;
        self
    }

    pub fn summary_rows(mut self, rows: u16) -> Self {
        self.summary_rows = rows;
        self
    }

    pub fn esc_returns(mut self, on: bool) -> Self {
        self.esc_returns = on;
        self
    }

    pub fn detail_owns(mut self, ids: &[WidgetId]) -> Self {
        self.detail_owns = ids.to_vec();
        self
    }

    pub fn in_detail(&self, id: WidgetId) -> bool {
        id == self.detail || self.detail_owns.contains(&id)
    }

    pub fn drawer_open(&self) -> bool {
        self.drawer
    }

    pub fn last(&self) -> MdLayout {
        self.last
    }

    pub fn toggle_drawer(&mut self, focus: &mut Focus) {
        self.drawer = !self.drawer;
        focus.focus(if self.drawer {
            self.detail
        } else {
            self.master
        });
    }

    pub fn open_drawer(&mut self, focus: &mut Focus) {
        self.drawer = true;
        focus.focus(self.detail);
    }

    pub fn close_drawer(&mut self, focus: &mut Focus) {
        self.drawer = false;
        focus.focus(self.master);
    }

    pub fn menu_item(&self) -> MenuItem {
        MenuItem::new(if self.drawer {
            "Hide details"
        } else {
            "Show details"
        })
    }

    pub fn layout(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) -> MdLayout {
        let mut l = MdLayout::default();
        if let Some((master, detail)) = self.split_rects(area) {
            l.mode = Mode::Split;
            l.master = master;
            l.detail = detail;
            if self.size == DetailSize::Resizable {
                self.container = area;
                let handle = self.split.handle(SplitDir::Horizontal, area, self.gap);
                if !handle.is_empty() {
                    let off = (self.gap / 2).min(handle.width.saturating_sub(1));
                    l.seam = Rect::new(handle.x + off, handle.y, 1, handle.height);
                }
                self.seam.render(l.seam, buf, ctx, bg);
            }
        } else {
            self.drawer = self.collapsed_mode(&ctx.interaction) == Mode::Drawer;
            if self.drawer {
                l.mode = Mode::Drawer;
                l.detail = area;

                ctx.control(self.master, Rect::ZERO, false);
            } else {
                l.mode = Mode::Master;
                let rows = self.summary_rows.min(area.height / 3);
                if rows == 0 {
                    l.master = area;
                } else {
                    l.master = Rect::new(
                        area.x,
                        area.y,
                        area.width,
                        area.height.saturating_sub(rows + 1),
                    );
                    l.summary = Rect::new(area.x, l.master.bottom() + 1, area.width, rows);
                }
            }
        }
        self.last = l;
        l
    }

    /// Drops the last frame's geometry. A host calls this when it renders
    /// into no space at all, so keys and drags cannot resize a split that is
    /// not on screen.
    pub(crate) fn forget_geometry(&mut self) {
        self.last = MdLayout::default();
        self.container = Rect::ZERO;
    }

    pub fn finish(&self, ctx: &mut RenderCtx) {
        if self.last.mode == Mode::Master {
            ctx.control(self.detail, Rect::ZERO, false);
        }
    }

    pub fn on_key(&mut self, key: &Key, focus: &mut Focus) -> Option<Outcome> {
        let f = focus.current()?;
        let on_master = f == self.master;
        let on_detail = self.in_detail(f);
        if !on_master && !on_detail {
            return None;
        }
        if self.last.mode == Mode::Split && self.size == DetailSize::Resizable {
            let (container, gap) = (self.container, self.gap);
            if let Some(o) =
                self.keys
                    .apply(key, &mut self.split, SplitDir::Horizontal, container, gap)
            {
                return Some(o);
            }
        }
        if on_detail && self.esc_returns && key.code == KeyCode::Esc && key.mods.is_empty() {
            self.close_drawer(focus);
            return Some(Outcome::Changed);
        }
        None
    }

    pub fn on_pointer(&mut self, ev: &PointerEvent) -> Option<Outcome> {
        if ev.id != self.seam.id || self.size != DetailSize::Resizable {
            return None;
        }
        let (container, gap) = (self.container, self.gap);
        match ev.phase()? {
            Phase::Press => {
                self.seam.on_press(&self.split, container, gap, ev.pos());
                Some(Outcome::Consumed)
            }
            Phase::Drag => Some(self.seam.on_drag(&mut self.split, container, gap, ev.pos())),
            Phase::Click => Some(self.seam.on_click(&mut self.split, ev.count())),
            _ => None,
        }
    }

    pub fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        let Some(f) = focus else { return Vec::new() };
        let on_detail = self.in_detail(f);
        if f != self.master && !on_detail {
            return Vec::new();
        }
        let mut v = Vec::new();
        if self.last.mode == Mode::Split
            && self.size == DetailSize::Resizable
            && let Some(h) = self.keys.hint(SplitDir::Horizontal)
        {
            v.push(h);
        }
        if on_detail && self.esc_returns {
            v.push(hint("Esc", "Back"));
        }
        v
    }
}

/// Master, gap and detail widths for a share layout in `width` columns, or
/// `None` when the minimum pane cannot fit beside the gap. The three widths sum
/// to `width`. `pct` is capped at 100 and `max` is raised to `min`; `Share`
/// floors its preferred width and `MasterShare` rounds it.
fn share_widths(
    width: u16,
    gap: u16,
    pct: u16,
    min: u16,
    max: u16,
    detail_share: bool,
) -> Option<(u16, u16, u16)> {
    let gap = gap.min(width);
    let usable = width - gap;
    if min > usable {
        return None;
    }
    let round = if detail_share { 0 } else { 50 };
    let preferred = (u32::from(width) * u32::from(pct.min(100)) + round) / 100;
    let pane = preferred.clamp(u32::from(min), u32::from(max.max(min))) as u16;
    let pane = pane.min(usable);
    let rest = usable - pane;
    Some(if detail_share {
        (rest, gap, pane)
    } else {
        (pane, gap, rest)
    })
}

impl crate::ui::specimen::Specimen for MasterDetail {
    fn specimen_id(&self) -> WidgetId {
        self.seam.id
    }

    fn supports(&self, state: crate::ui::specimen::SpecimenState) -> bool {
        use crate::ui::specimen::SpecimenState::*;
        matches!(state, Rest | Hover | Pressed | Narrow)
    }

    fn specimen_height(&self, _state: crate::ui::specimen::SpecimenState) -> u16 {
        3
    }

    fn specimen(
        &self,
        state: crate::ui::specimen::SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        use crate::ui::specimen::SpecimenState;
        let t = ctx.theme;
        let mut md = self.clone();
        md.summary_rows = 1;
        md.collapse_below = if state == SpecimenState::Narrow {
            u16::MAX
        } else {
            0
        };
        md.split = Split::new(45, 4, 4);
        md.size = DetailSize::Resizable;
        md.gap = 1;
        let l = ctx.visual_order(|ctx| {
            let inert = std::mem::replace(&mut ctx.inert, true);
            let l = md.layout(area, buf, ctx, bg);
            ctx.inert = inert;
            l
        });
        let line = |buf: &mut Buffer, r: Rect, text: &str, style| {
            if !r.is_empty() {
                buf.set_string(
                    r.x,
                    r.y,
                    crate::ui::text::truncate(text, r.width as usize),
                    style,
                );
            }
        };
        let rows = ["▸ orders", "  users", "  events"];
        for (i, row) in rows.iter().enumerate() {
            let r = l.master;
            if (i as u16) < r.height {
                line(
                    buf,
                    Rect::new(r.x, r.y + i as u16, r.width, 1),
                    row,
                    t.secondary().bg(bg),
                );
            }
        }
        let facts = ["rows  12,408", "size  3.1 MB"];
        for (i, fact) in facts.iter().enumerate() {
            let r = l.detail;
            if (i as u16) < r.height {
                line(
                    buf,
                    Rect::new(r.x + 1, r.y + i as u16, r.width.saturating_sub(1), 1),
                    fact,
                    t.muted().bg(bg),
                );
            }
        }
        line(buf, l.summary, "→ orders · 12,408 rows", t.muted().bg(bg));
    }
}
