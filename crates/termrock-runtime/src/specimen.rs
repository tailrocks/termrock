use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::ctx::{Interaction, RenderCtx, VisualState};
use termrock_core::focus::FocusRing;
use termrock_core::hit::HitRegistry;
use termrock_core::id::WidgetId;
use termrock_theme::{ColorLevel, Kind, Plane, Theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecimenState {
    Rest,

    Hover,

    Pressed,

    Focused,

    FocusedHover,

    Selected,

    Disabled,

    Busy,

    Error,

    Editing,

    Empty,

    Loading,

    Overflow,

    Narrow,
}

impl SpecimenState {
    pub const ALL: [Self; 14] = [
        Self::Rest,
        Self::Hover,
        Self::Pressed,
        Self::Focused,
        Self::FocusedHover,
        Self::Selected,
        Self::Disabled,
        Self::Busy,
        Self::Error,
        Self::Editing,
        Self::Empty,
        Self::Loading,
        Self::Overflow,
        Self::Narrow,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            Self::Rest => "rest",
            Self::Hover => "hover",
            Self::Pressed => "pressed",
            Self::Focused => "focused",
            Self::FocusedHover => "focused_hover",
            Self::Selected => "selected",
            Self::Disabled => "disabled",
            Self::Busy => "busy",
            Self::Error => "error",
            Self::Editing => "editing",
            Self::Empty => "empty",
            Self::Loading => "loading",
            Self::Overflow => "overflow",
            Self::Narrow => "narrow",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::FocusedHover => "focus + hover",
            other => other.slug(),
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.slug() == slug)
    }

    pub fn visual(self) -> VisualState {
        let mut v = VisualState::default();
        match self {
            Self::Hover => v.hovered = true,

            Self::Pressed => {
                v.hovered = true;
                v.pressed = true;
            }
            Self::Focused => v.focused = true,
            Self::FocusedHover => {
                v.focused = true;
                v.hovered = true;
            }
            Self::Selected => v.selected = true,
            Self::Disabled => v.disabled = true,
            Self::Busy => v.busy = true,
            Self::Error => v.error = true,
            Self::Editing => {
                v.focused = true;
                v.editing = true;
            }
            Self::Rest | Self::Empty | Self::Loading | Self::Overflow | Self::Narrow => {}
        }
        v
    }

    fn interaction(self, id: WidgetId, tick: u64) -> Interaction {
        let v = self.visual();
        Interaction {
            focus: v.focused.then_some(id),
            hover: v.hovered.then_some(id),
            pressed: v.pressed.then_some(id),
            tick,
            ..Default::default()
        }
    }
}

pub trait Specimen {
    fn specimen_id(&self) -> WidgetId;

    fn supports(&self, state: SpecimenState) -> bool;

    fn specimen_height(&self, _state: SpecimenState) -> u16 {
        1
    }

    fn specimen(
        &self,
        state: SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    );
}

const LEVELS: [ColorLevel; 4] = [
    ColorLevel::TrueColor,
    ColorLevel::Ansi256,
    ColorLevel::Ansi16,
    ColorLevel::Mono,
];

pub struct StateGrid {
    pub id: WidgetId,

    pub rows: Vec<SpecimenState>,
    columns: Vec<(String, Box<dyn Specimen>)>,

    pub label_width: u16,

    pub column_width: u16,

    pub narrow_width: u16,

    pub focus_mark: Option<&'static str>,

    rendered: Vec<SpecimenState>,
}

impl std::fmt::Debug for StateGrid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateGrid")
            .field("id", &self.id)
            .field("rows", &self.rows)
            .field(
                "columns",
                &self.columns.iter().map(|(l, _)| l).collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl crate::measure::Measure for StateGrid {
    fn measure(&self, _avail: ratatui::layout::Size, _ctx: &RenderCtx) -> ratatui::layout::Size {
        let (w, h) = self.size();
        ratatui::layout::Size::new(w, h)
    }
}

impl StateGrid {
    pub fn new(id: WidgetId) -> Self {
        Self {
            id,
            rows: Vec::new(),
            columns: Vec::new(),
            label_width: 15,
            column_width: 15,
            narrow_width: 24,
            focus_mark: None,
            rendered: Vec::new(),
        }
    }

    pub fn row(mut self, state: SpecimenState) -> Self {
        self.rows.push(state);
        self
    }

    pub fn rows(mut self, states: &[SpecimenState]) -> Self {
        self.rows.extend_from_slice(states);
        self
    }

    pub fn column(mut self, label: &str, specimen: impl Specimen + 'static) -> Self {
        self.columns.push((label.to_owned(), Box::new(specimen)));
        self
    }

    pub fn label_width(mut self, w: u16) -> Self {
        self.label_width = w;
        self
    }

    pub fn column_width(mut self, w: u16) -> Self {
        self.column_width = w;
        self
    }

    pub fn narrow_width(mut self, w: u16) -> Self {
        self.narrow_width = w;
        self
    }

    pub fn focus_mark(mut self, glyph: &'static str) -> Self {
        self.focus_mark = Some(glyph);
        self
    }

    pub fn column_labels(&self) -> impl Iterator<Item = &str> {
        self.columns.iter().map(|(l, _)| l.as_str())
    }

    pub fn rendered_rows(&self) -> &[SpecimenState] {
        &self.rendered
    }

    fn row_height(&self, state: SpecimenState) -> u16 {
        self.columns
            .iter()
            .filter(|(_, s)| s.supports(state))
            .map(|(_, s)| s.specimen_height(state))
            .max()
            .unwrap_or(1)
            .max(1)
    }

    pub fn size(&self) -> (u16, u16) {
        let w = self.label_width + self.column_width * self.columns.len() as u16;
        let h = 1 + self.rows.iter().map(|&r| self.row_height(r)).sum::<u16>();
        (w, h)
    }

    pub fn mark(&self, t: &Theme) -> &'static str {
        self.focus_mark.unwrap_or(if t.glyphs.focus_bar.is_ascii() {
            ":"
        } else {
            "▏"
        })
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        self.render_rows(0..self.rows.len(), true, area, buf, ctx, bg);
    }

    pub fn row_heights(&self) -> Vec<u16> {
        self.rows.iter().map(|&r| self.row_height(r)).collect()
    }

    pub fn render_rows(
        &mut self,
        rows: std::ops::Range<usize>,
        header: bool,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        self.rendered.clear();
        let tick = ctx.interaction.tick;
        let theme = ctx.theme;
        self.draw_rows(theme, tick, rows, header, area, buf, bg);
    }

    pub fn levels_height(&self, width: u16) -> u16 {
        let (w, h) = self.size();
        let per_row = Self::per_row(w, width);
        let bands = LEVELS.len().div_ceil(per_row) as u16;

        bands * (h + 1) + bands.saturating_sub(1)
    }

    fn per_row(grid_w: u16, width: u16) -> usize {
        [4usize, 2]
            .into_iter()
            .find(|&n| grid_w * n as u16 + 2 * (n as u16 - 1) <= width)
            .unwrap_or(1)
    }

    pub fn render_levels(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let (w, h) = self.size();
        let per_row = Self::per_row(w, area.width);
        let area = area.intersection(buf.area);
        let mut drawn: Option<Vec<SpecimenState>> = None;
        for (i, level) in LEVELS.into_iter().enumerate() {
            let col = (i % per_row) as u16;
            let band = (i / per_row) as u16;
            let x = area.x + col * (w + 2);
            let y = area.y + band * (h + 2);
            let pane =
                Rect::new(x, y, w.min(area.right().saturating_sub(x)), h + 1).intersection(area);
            if pane.is_empty() {
                continue;
            }
            self.render_level(level, pane, buf, ctx);

            let rows = std::mem::take(&mut self.rendered);
            drawn = Some(match drawn {
                None => rows,
                Some(mut d) => {
                    d.retain(|r| rows.contains(r));
                    d
                }
            });
        }
        self.rendered = drawn.unwrap_or_default();
    }

    pub fn render_level(
        &mut self,
        level: ColorLevel,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
    ) {
        self.rendered.clear();
        let tick = ctx.interaction.tick;
        let pane = area.intersection(buf.area);
        if pane.is_empty() {
            return;
        }
        let own;
        let t: &Theme = if ctx.theme.level == level {
            ctx.theme
        } else {
            own = Theme::for_level(level);
            &own
        };
        let bg = t.surface;
        crate::ctx::fill(buf, pane, Style::new().bg(bg));
        buf.set_string(pane.x, pane.y, level.label(), t.muted().bg(bg));
        let body = Rect::new(
            pane.x,
            pane.y + 1,
            pane.width,
            pane.height.saturating_sub(1),
        );
        self.draw(t, tick, body, buf, bg);
    }

    fn draw(&mut self, t: &Theme, tick: u64, area: Rect, buf: &mut Buffer, bg: Color) {
        self.draw_rows(t, tick, 0..self.rows.len(), true, area, buf, bg);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_rows(
        &mut self,
        t: &Theme,
        tick: u64,
        rows: std::ops::Range<usize>,
        header: bool,
        area: Rect,
        buf: &mut Buffer,
        bg: Color,
    ) {
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return;
        }
        let mark = self.mark(t);
        let col_x = |k: usize| area.x + self.label_width + k as u16 * self.column_width;
        let fits = |k: usize| col_x(k) + self.column_width <= area.right() + 1;
        let mut y = area.y;
        if header {
            for (k, (label, _)) in self.columns.iter().enumerate() {
                if !fits(k) {
                    break;
                }
                let room = self.column_width.saturating_sub(1) as usize;
                buf.set_string(
                    col_x(k),
                    area.y,
                    termrock_text::cells::fit(label, room),
                    t.muted().bg(bg),
                );
            }
            y += 1;
        }
        let end = rows.end.min(self.rows.len());
        let start = rows.start.min(end);
        for i in start..end {
            let state = self.rows[i];
            let h = self.row_height(state);
            if y + h > area.bottom() {
                break;
            }
            buf.set_string(
                area.x,
                y,
                termrock_text::cells::fit(
                    state.label(),
                    self.label_width.saturating_sub(1) as usize,
                ),
                t.secondary().bg(bg),
            );
            for (k, (_, specimen)) in self.columns.iter().enumerate() {
                if !fits(k) {
                    break;
                }
                let mut w = self.column_width.saturating_sub(1);
                if state == SpecimenState::Narrow {
                    w = w.min(self.narrow_width);
                }
                let cell = Rect::new(col_x(k), y, w, h).intersection(area);
                if !specimen.supports(state) {
                    buf.set_string(cell.x + 1, cell.y, "–", t.faint().bg(bg));
                    continue;
                }
                let mut hits = HitRegistry::default();
                let mut ring = FocusRing::default();
                let interaction = state.interaction(specimen.specimen_id(), tick);
                let mut sctx = RenderCtx::detached(t, interaction, &mut hits, &mut ring);
                specimen.specimen(state, cell, buf, &mut sctx, bg);
                depict_focus(buf, cell, t, mark, bg);
            }
            self.rendered.push(state);
            y += h;
        }
    }
}

fn depict_focus(buf: &mut Buffer, area: Rect, t: &Theme, mark: &str, bg: Color) {
    let bar = t.glyphs.focus_bar;
    for pos in area.positions() {
        let Some(c) = buf.cell_mut(pos) else {
            continue;
        };

        let live =
            c.symbol() == bar && (!bar.is_ascii() || c.fg == t.focus || c.fg == t.text_on_accent);
        if !live {
            continue;
        }
        c.set_symbol(mark);
        if c.bg == bg {
            c.fg = t.text_muted;
        }
    }
}

#[derive(Debug, Clone)]
pub struct KindSample {
    pub id: WidgetId,
    pub kind: Kind,

    pub plane: Plane,
    pub label: String,
}

impl KindSample {
    pub fn new(id: WidgetId, kind: Kind, plane: Plane, label: &str) -> Self {
        Self {
            id,
            kind,
            plane,
            label: label.to_owned(),
        }
    }

    pub fn plane(mut self, plane: Plane) -> Self {
        self.plane = plane;
        self
    }

    pub fn draw(&self, t: &Theme, s: VisualState, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return;
        }
        let plane_bg = t.ladder(self.plane).rest;
        crate::ctx::fill(buf, area, Style::new().bg(plane_bg));
        let r = t.interactive(self.kind, s, self.plane);
        let row = Rect::new(area.x, area.y, area.width, 1);

        let body = Style::new().bg(plane_bg).patch(r.style);
        let gutter = Style::new().bg(plane_bg).patch(r.gutter_style);
        buf.set_string(row.x, row.y, r.gutter, gutter);
        let w = termrock_text::cells::width(r.gutter) as u16;
        let room = row.width.saturating_sub(w) as usize;
        let text = termrock_text::cells::fit(&format!(" {} ", self.label), room);
        let tw = termrock_text::cells::width(&text) as u16;
        crate::ctx::fill(buf, Rect::new(row.x + w, row.y, tw, 1), body);
        buf.set_string(row.x + w, row.y, text, body);
    }
}

impl Specimen for KindSample {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: SpecimenState) -> bool {
        use SpecimenState::*;
        matches!(
            state,
            Rest | Hover | Pressed | Focused | FocusedHover | Selected | Disabled | Error
        )
    }

    fn specimen(
        &self,
        state: SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        _bg: Color,
    ) {
        self.draw(ctx.theme, state.visual(), area, buf);
    }
}
