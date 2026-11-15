use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Style};

use crate::core::focus::FocusRing;
use crate::core::hit::HitRegistry;
use crate::core::id::WidgetId;
use crate::theme::{ColorLevel, Kind, Plane, Theme};
use crate::ui::ctx::{Interaction, RenderCtx, VisualState};

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

impl crate::ui::measure::Measure for StateGrid {
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
        crate::ui::ctx::fill(buf, pane, Style::new().bg(bg));
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
                    crate::ui::text::fit(label, room),
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
                crate::ui::text::fit(state.label(), self.label_width.saturating_sub(1) as usize),
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
        crate::ui::ctx::fill(buf, area, Style::new().bg(plane_bg));
        let r = t.interactive(self.kind, s, self.plane);
        let row = Rect::new(area.x, area.y, area.width, 1);

        let body = Style::new().bg(plane_bg).patch(r.style);
        let gutter = Style::new().bg(plane_bg).patch(r.gutter_style);
        buf.set_string(row.x, row.y, r.gutter, gutter);
        let w = crate::ui::text::width(r.gutter) as u16;
        let room = row.width.saturating_sub(w) as usize;
        let text = crate::ui::text::fit(&format!(" {} ", self.label), room);
        let tw = crate::ui::text::width(&text) as u16;
        crate::ui::ctx::fill(buf, Rect::new(row.x + w, row.y, tw, 1), body);
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

impl Specimen for crate::widgets::button::Button {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: SpecimenState) -> bool {
        use SpecimenState::*;
        match state {
            Rest | Hover | Pressed | Focused | FocusedHover | Disabled | Busy | Overflow => true,
            Selected => self.on.is_some(),
            Error | Editing | Empty | Loading | Narrow => false,
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
        let mut b = self.clone();
        b.tip = None;
        let v = state.visual();
        b.disabled |= v.disabled;
        b.busy |= v.busy;
        if v.selected {
            b.on = Some(true);
        }
        if state == SpecimenState::Overflow {
            b.label = format!("{} with a long label", b.label);
        }
        b.render(area, buf, ctx, bg);
    }
}

impl Specimen for crate::widgets::input::TextInput {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: SpecimenState) -> bool {
        use SpecimenState::*;
        match state {
            Rest | Hover | Focused | FocusedHover | Editing | Error | Disabled | Empty
            | Overflow | Narrow => true,
            Pressed | Selected | Busy | Loading => false,
        }
    }

    fn specimen_height(&self, state: SpecimenState) -> u16 {
        if state == SpecimenState::Error || self.field.error.is_some() {
            3
        } else {
            2
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
        let mut f = self.clone();
        f.help.clear();
        let v = state.visual();
        f.disabled |= v.disabled;
        f.editing = v.editing;
        if v.error && f.field.error.is_none() {
            f.field.error = Some("Enter a valid value".into());
        }
        match state {
            SpecimenState::Empty => {
                if f.placeholder.is_empty() {
                    f.placeholder = "Type a value…".into();
                }
                f = f.value("");
            }
            SpecimenState::Overflow => {
                let long = format!(
                    "{} and a value longer than the field",
                    f.field.buffer.text()
                );
                f = f.value(&long);
            }
            _ => {}
        }
        f.render(area, buf, ctx, bg);

        if v.editing && f.area.width > 3 {
            let field = f.area;
            let text_w = crate::ui::text::width(f.field.buffer.text()) as u16;
            let x = (field.x + 2 + text_w).min(field.right().saturating_sub(2));
            if let Some(c) = buf.cell_mut(Position::new(x, field.y)) {
                c.bg = ctx.theme.text_primary;
            }
        }
    }
}

impl Specimen for crate::widgets::tabs::Tabs {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: SpecimenState) -> bool {
        use SpecimenState::*;
        match state {
            Rest | Hover | Pressed | Focused | FocusedHover | Busy | Error | Overflow | Narrow => {
                !self.items.is_empty()
            }
            Selected | Disabled | Editing | Empty | Loading => false,
        }
    }

    fn specimen_height(&self, _state: SpecimenState) -> u16 {
        2
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
        let v = state.visual();
        let target = (t.active + 1).min(t.items.len().saturating_sub(1));
        if v.hovered {
            ctx.interaction.hover = Some(t.tab_id(target));
        }
        if v.pressed {
            ctx.interaction.pressed = Some(t.tab_id(target));
        }
        if v.focused {
            t.cursor = if v.hovered { t.active } else { target };
        }
        if let Some(it) = t.items.get_mut(target) {
            it.busy |= v.busy;
            it.error |= v.error;
        }
        if state == SpecimenState::Overflow {
            let mut items = t.items.clone();
            items.extend((1..=6).map(|i| crate::widgets::tabs::TabItem::new(&format!("More {i}"))));
            t.set_items(items);
        }
        t.render(area, buf, ctx, bg);
    }
}

impl Specimen for crate::widgets::menu::ContextMenu {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: SpecimenState) -> bool {
        use SpecimenState::*;
        match state {
            Rest | Hover | Pressed | Disabled | Overflow | Narrow => self.items.len() > 1,
            Focused | FocusedHover | Selected | Busy | Error | Editing | Empty | Loading => false,
        }
    }

    fn specimen_height(&self, state: SpecimenState) -> u16 {
        let mut m = self.clone();
        if state == SpecimenState::Overflow {
            m = m.max_rows(2);
        }
        m.size().1
    }

    fn specimen(
        &self,
        state: SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        let _ = bg;
        let mut m = self
            .clone()
            .anchor(
                Rect::new(area.x, area.y, 1, 0),
                crate::ui::popup::Placement::Below,
            )
            .edge_margin(0);
        m.anchor_to = None;
        let v = state.visual();
        let row = 1.min(m.items.len() - 1);
        if v.disabled {
            m.items[row].item.disabled = true;
        }
        if v.hovered {
            ctx.interaction.hover = Some(m.row_id(row));
        }
        if v.pressed {
            ctx.interaction.pressed = Some(m.row_id(row));
        }
        if state == SpecimenState::Overflow {
            m = m.max_rows(2);
        }
        m.render(area, buf, ctx);
        ctx.flush_overlays(buf);
    }
}

impl Specimen for crate::widgets::dialog::Dialog {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: SpecimenState) -> bool {
        use SpecimenState::*;
        match state {
            Rest | Hover | Pressed | Focused | FocusedHover | Disabled | Overflow => {
                !self.actions.is_empty()
            }
            Editing => dialog_field(self).is_some(),
            Selected | Busy | Error | Empty | Loading | Narrow => false,
        }
    }

    fn specimen_height(&self, _state: SpecimenState) -> u16 {
        self.height(self.width)
    }

    fn specimen(
        &self,
        state: SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        use crate::widgets::dialog::DialogBody;
        let _ = bg;
        let mut d = self.clone();
        d.outer_margin = ratatui::layout::Margin::new(0, 0);
        d.placement = crate::ui::popup::Placement::Center;
        d.backdrop = crate::ui::ctx::Backdrop::FULL;
        d.width = area.width;
        let target = dialog_target(&d);
        let v = state.visual();
        if v.disabled
            && let Some(b) = d.actions.get_mut(target)
        {
            b.disabled = true;
        }
        if state == SpecimenState::Overflow {
            d.max_height = Some(self.height(self.width));
            if let DialogBody::Text(t) = &mut d.body {
                t.push_str(
                    " The rest of this text is longer than the dialog has room for, \
                     so the body scrolls and says how much is below.",
                );
            }
        }
        let field = dialog_field(&d);
        if v.editing
            && let DialogBody::Input(i)
            | DialogBody::Facts {
                ack: Some(crate::widgets::dialog::AckInput { input: i, .. }),
                ..
            } = &mut d.body
        {
            i.editing = true;
        }
        let button = d.actions.get(target).map(|b| b.id);
        ctx.interaction.hover = if v.hovered { button } else { None };
        ctx.interaction.pressed = if v.pressed { button } else { None };
        ctx.interaction.focus = match (v.focused, v.editing) {
            (true, true) => field,
            (true, false) => button,
            _ => None,
        };
        d.render_in(area, area, buf, ctx);
    }
}

fn dialog_target(d: &crate::widgets::dialog::Dialog) -> usize {
    d.confirm_index
        .or_else(|| {
            d.actions.iter().position(|b| {
                matches!(
                    b.kind,
                    crate::theme::ButtonKind::Primary | crate::theme::ButtonKind::Danger
                )
            })
        })
        .unwrap_or(d.actions.len().saturating_sub(1))
}

fn dialog_field(d: &crate::widgets::dialog::Dialog) -> Option<WidgetId> {
    use crate::widgets::dialog::DialogBody;
    match &d.body {
        DialogBody::Input(i) => Some(i.id),
        DialogBody::Facts { ack: Some(a), .. } => Some(a.input.id),
        _ => None,
    }
}

impl Specimen for crate::widgets::choice_dialog::ChoiceDialog {
    fn specimen_id(&self) -> WidgetId {
        self.radio.id
    }

    fn supports(&self, state: SpecimenState) -> bool {
        use SpecimenState::*;
        match state {
            Rest | Hover | Pressed | Focused | FocusedHover | Selected | Disabled => {
                !self.buttons.is_empty()
            }
            Busy | Error | Editing | Empty | Loading | Overflow | Narrow => false,
        }
    }

    fn specimen_height(&self, _state: SpecimenState) -> u16 {
        self.height() + 2
    }

    fn specimen(
        &self,
        state: SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        let _ = bg;
        let mut d = self.clone();
        let primary = (0..d.buttons.len())
            .rev()
            .find(|&i| i != d.cancel_index)
            .unwrap_or(0);
        let v = state.visual();
        if v.disabled {
            d.buttons[primary].disabled = true;
        }
        if v.selected {
            let last = d.radio.options.len().saturating_sub(1);
            d.radio.selected = last;
            d.radio.cursor = last;
        }
        let button = d.buttons.get(primary).map(|b| b.id);
        ctx.interaction.hover = if v.hovered { button } else { None };
        ctx.interaction.pressed = if v.pressed { button } else { None };
        ctx.interaction.focus = match (v.focused, v.hovered) {
            (true, false) => Some(d.radio.id),
            (true, true) => button,
            _ => None,
        };
        d.render(area, buf, ctx);
    }
}

impl Specimen for crate::ui::modal::Frame<'static> {
    fn specimen_id(&self) -> WidgetId {
        crate::ui::modal::CLOSE
    }

    fn supports(&self, state: SpecimenState) -> bool {
        use SpecimenState::*;
        let close = self.dismiss.is_some_and(|d| d.shows_close());
        match state {
            Rest => true,
            Hover | Pressed => close,
            _ => false,
        }
    }

    fn specimen_height(&self, _state: SpecimenState) -> u16 {
        let m = crate::widgets::dialog::OUTER_MARGIN;
        self.height.max(self.min_height) + 2 * m.vertical + self.top + self.bottom + 2
    }

    fn specimen(
        &self,
        state: SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        let mut f = self.clone();
        if f.placement != crate::ui::popup::Placement::Center {
            let anchor = Rect::new(area.x + 4, area.y + f.top + 1, 8, 1).intersection(area);
            f.anchor = anchor;
            let t = ctx.theme;
            buf.set_string(anchor.x, anchor.y, "▣ anchor", t.secondary().bg(bg));
        }
        let v = state.visual();
        ctx.interaction.focus = None;
        ctx.interaction.hover = v.hovered.then_some(crate::ui::modal::CLOSE);
        ctx.interaction.pressed = v.pressed.then_some(crate::ui::modal::CLOSE);
        f.render(area, buf, ctx);
    }
}

impl Specimen for crate::widgets::picker::Picker {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: SpecimenState) -> bool {
        use SpecimenState::*;
        match state {
            Rest | Empty | Loading | Error => true,
            Hover | Pressed | Disabled => self.visible_len() > 1,
            Overflow => self.visible_len() > 3,
            Focused | FocusedHover | Selected | Busy | Editing | Narrow => false,
        }
    }

    fn specimen_height(&self, state: SpecimenState) -> u16 {
        picker_sample(self, state).height() + 2
    }

    fn specimen(
        &self,
        state: SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        let _ = bg;
        let mut p = picker_sample(self, state);
        let w = p.list().width.1.min(area.width);
        p.set_width(w);
        let v = state.visual();
        let row = p.row_id(1.min(p.visible_len().saturating_sub(1)));
        ctx.interaction.focus = None;
        ctx.interaction.hover = v.hovered.then_some(row);
        ctx.interaction.pressed = v.pressed.then_some(row);
        p.draw(area, buf, ctx);
    }
}

fn picker_sample(
    p: &crate::widgets::picker::Picker,
    state: SpecimenState,
) -> crate::widgets::picker::Picker {
    use crate::widgets::picker::PickerStatus;
    let mut p = p.clone();
    p.set_placement(crate::ui::popup::Placement::Center);
    p.anchor = Rect::ZERO;
    p.backdrop = crate::ui::ctx::Backdrop::FULL;
    match state {
        SpecimenState::Disabled => {
            let mut items: Vec<_> = p
                .visible_items()
                .map(|v| v.item.clone().matched(v.matched.to_vec()))
                .collect();
            if let Some(item) = items.get_mut(1) {
                item.disabled = true;
            }
            p.set_items(items)
                .unwrap_or_else(|error| p.report_item_error(error));
        }
        SpecimenState::Empty => {
            p.set_items(vec![])
                .unwrap_or_else(|error| p.report_item_error(error));
        }
        SpecimenState::Loading => {
            p.set_items(vec![])
                .unwrap_or_else(|error| p.report_item_error(error));
            p.status = PickerStatus::Loading("Fetching branches…".into());
        }
        SpecimenState::Error => {
            p.set_items(vec![])
                .unwrap_or_else(|error| p.report_item_error(error));
            p.status = PickerStatus::Error {
                message: "Could not reach origin".into(),
                detail: Some("r retries".into()),
            };
        }
        SpecimenState::Overflow => p.set_max_rows(3),
        _ => {}
    }
    p
}
