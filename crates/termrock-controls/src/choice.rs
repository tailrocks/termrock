use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::core::event::{Key, Outcome};
use crate::core::id::{ItemKey, Part, PartRef, WidgetId};
use crate::core::keymap::{Nav, NavKeys};
use crate::core::list::{Cursor, Navigable, RowSource};
use crate::core::refusal::{Control, Reason};
use crate::ui::ctx::RenderCtx;
use crate::ui::measure::terminal_extent;
use crate::ui::row::{RowLook, RowUi};

fn row_part(buf: &mut Buffer, row: Rect, offset: u16, text: &str, style: Style) {
    if offset < row.width {
        buf.set_stringn(
            row.x + offset,
            row.y,
            text,
            usize::from(row.width - offset),
            style,
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CheckHit {
    #[default]
    Label,

    Row,
}

#[derive(Debug, Clone)]
pub struct Checkbox {
    pub id: WidgetId,
    pub label: String,
    pub checked: bool,
    pub disabled: bool,
    pub control: Control,

    pub mixed: bool,

    pub hit: CheckHit,
    pub area: Rect,
}

impl Checkbox {
    pub fn new(id: WidgetId, label: &str, checked: bool) -> Self {
        Self {
            id,
            label: label.to_owned(),
            checked,
            disabled: false,
            control: Control::Live,
            mixed: false,
            hit: CheckHit::Label,
            area: Rect::ZERO,
        }
    }

    pub fn control(mut self, control: Control) -> Self {
        self.control = control;
        self
    }

    pub fn fixed(self, reason: Reason) -> Self {
        self.control(Control::fixed(reason))
    }

    pub fn hit(mut self, hit: CheckHit) -> Self {
        self.hit = hit;
        self
    }

    pub fn mixed(mut self, mixed: bool) -> Self {
        self.mixed = mixed;
        self
    }

    fn toggle(&mut self) {
        self.checked = self.mixed || !self.checked;
        self.mixed = false;
    }

    pub fn on_key(&mut self, key: &Key) -> Outcome {
        if self.disabled {
            return Outcome::Ignored;
        }
        if key.is_char(' ') || key.is(KeyCode::Enter) {
            if self.control.refuse() {
                return Outcome::Consumed;
            }
            self.toggle();
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    pub fn on_click(&mut self) -> Outcome {
        if self.disabled || self.control.refuse() {
            return Outcome::Consumed;
        }
        self.toggle();
        Outcome::Changed
    }

    /// Natural width of the checkbox row, saturating at `u16::MAX`.
    pub fn width(&self) -> u16 {
        terminal_extent(crate::ui::text::width(&self.label).saturating_add(6))
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        let area = area.intersection(*buf.area());
        let area = Rect::new(area.x, area.y, area.width, 1.min(area.height));
        self.area = area;
        if area.is_empty() {
            return;
        }
        let t = ctx.theme;
        let mut s = ctx.state(self.id);
        s.fixed = self.control.is_fixed();
        s.disabled = self.disabled || s.fixed;
        s.selected = false;
        if s.disabled {
            s.hovered = false;
        }
        let st = RowUi::paint(ctx, buf, area, s, RowLook::Bar, bg).style();
        let g = &t.glyphs;
        let mark = match (area.width < 4, self.checked, self.mixed) {
            (true, _, true) => g.dash,
            (false, _, true) => g.check_mixed,
            (true, true, _) => g.tick,
            (true, false, _) => g.check_off_narrow,
            (false, true, _) => g.check_on,
            (false, false, _) => g.check_off,
        };
        let mark_style = if s.disabled {
            st
        } else if self.checked || self.mixed {
            st.fg(t.chosen_in(s))
        } else {
            st.fg(t.text_muted)
        };
        row_part(buf, area, 1, mark, mark_style);
        let label = crate::ui::text::truncate(&self.label, area.width.saturating_sub(6) as usize);
        row_part(buf, area, 5, &label, st);

        let target = match self.hit {
            CheckHit::Row => area,
            CheckHit::Label => {
                let w = 6 + crate::ui::text::width(&label) as u16;
                Rect::new(area.x, area.y, w.min(area.width), area.height)
            }
        };
        ctx.control(self.id, target, self.disabled);
    }
}

#[derive(Debug, Clone)]
pub struct RadioGroup {
    pub id: WidgetId,
    pub label: String,
    pub options: Vec<String>,
    pub selected: usize,
    pub cursor: usize,
    pub disabled: bool,
    pub controls: Vec<Control>,
    pub areas: Vec<Rect>,

    pub nav: NavKeys,
}

impl RadioGroup {
    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.nav = nav;
        self
    }

    pub fn new(id: WidgetId, label: &str, options: &[&str], selected: usize) -> Self {
        Self {
            id,
            label: label.to_owned(),
            options: options.iter().map(|s| (*s).to_owned()).collect(),
            selected,
            cursor: selected,
            disabled: false,
            controls: vec![Control::Live; options.len()],
            areas: vec![],
            nav: NavKeys::default(),
        }
    }

    pub fn option_control(mut self, i: usize, control: Control) -> Self {
        if let Some(slot) = self.controls.get_mut(i) {
            *slot = control;
        }
        self
    }

    pub fn fixed_option(self, i: usize, reason: Reason) -> Self {
        self.option_control(i, Control::fixed(reason))
    }

    fn fixed_at(&self, i: usize) -> bool {
        self.controls.get(i).is_some_and(Control::is_fixed)
    }

    fn refuse_at(&self, i: usize) -> bool {
        self.controls.get(i).is_some_and(Control::refuse)
    }

    pub fn height(&self) -> u16 {
        self.options.len().saturating_add(1).min(u16::MAX as usize) as u16
    }

    pub fn on_key(&mut self, key: &Key) -> Outcome {
        if self.disabled {
            return Outcome::Ignored;
        }
        let to = match self.nav.action(key) {
            Some(nav @ (Nav::Up | Nav::Down | Nav::Home | Nav::End)) if key.plain() => {
                self.cursor().apply(nav, self, 1).index()
            }
            _ => None,
        };
        if let Some(to) = to {
            self.cursor = to;
            if !self.fixed_at(to) {
                self.selected = to;
            }
            return Outcome::Changed;
        }
        match key.code {
            KeyCode::Char(' ') | KeyCode::Enter if key.plain() => {
                if self.refuse_at(self.cursor) {
                    return Outcome::Consumed;
                }
                self.selected = self.cursor;
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    pub fn on_click(&mut self, index: usize) -> Outcome {
        if self.disabled || index >= self.options.len() {
            return Outcome::Consumed;
        }
        self.cursor = index;
        if self.refuse_at(index) {
            return Outcome::Changed;
        }
        self.selected = index;
        Outcome::Changed
    }

    pub fn option_id(&self, i: usize) -> WidgetId {
        self.id.part(Self::option_part(i))
    }

    pub fn option_part(i: usize) -> PartRef {
        PartRef::item(Part::ROW, ItemKey::index(i))
    }

    /// Natural width of the group, saturating at `u16::MAX`.
    pub fn width(&self) -> u16 {
        let w = crate::ui::text::width;
        let options = self
            .options
            .iter()
            .map(|o| w(o).saturating_add(6))
            .max()
            .unwrap_or(0);
        terminal_extent(w(&self.label).saturating_add(2).max(options))
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        self.areas.clear();
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            return;
        }
        let t = ctx.theme;
        let focused = ctx.interaction.focused(self.id);
        let label_style = if self.disabled {
            t.faint().bg(bg)
        } else {
            t.label(focused).bg(bg)
        };
        row_part(
            buf,
            area,
            2,
            &crate::ui::text::truncate(&self.label, area.width.saturating_sub(2) as usize),
            label_style,
        );
        for (i, opt) in self
            .options
            .iter()
            .take(area.height.saturating_sub(1) as usize)
            .enumerate()
        {
            let y = area.y + 1 + i as u16;
            let row = Rect::new(area.x, y, area.width, 1);
            self.areas.push(row);
            let mut s = ctx.child_state(self.id, self.option_id(i), i == self.cursor);
            s.focused = focused && i == self.cursor;
            s.fixed = self.fixed_at(i);
            s.disabled = self.disabled || s.fixed;
            let st = RowUi::paint(ctx, buf, row, s, RowLook::Bar, bg).style();
            let on = i == self.selected;
            let g = &t.glyphs;
            let mark = match (row.width < 4, on) {
                (true, true) => g.dot_on,
                (true, false) => g.dot_off,
                (false, true) => g.radio_on,
                (false, false) => g.radio_off,
            };
            let ms = if s.disabled {
                st
            } else if on {
                st.fg(ctx.chosen(focused))
            } else {
                st.fg(t.text_muted)
            };
            row_part(buf, row, 1, mark, ms);
            row_part(
                buf,
                row,
                5,
                &crate::ui::text::truncate(opt, row.width.saturating_sub(5) as usize),
                st,
            );
            ctx.register_part(self.id, Self::option_part(i), row);
        }

        if !ctx.inert && !self.disabled && !self.areas.is_empty() {
            ctx.ring.register(self.id);
        }
    }
}

#[derive(Debug, Clone)]
pub struct Toggle {
    pub id: WidgetId,
    pub label: String,
    pub on: bool,
    pub disabled: bool,
    pub area: Rect,
}

impl Toggle {
    pub fn new(id: WidgetId, label: &str, on: bool) -> Self {
        Self {
            id,
            label: label.to_owned(),
            on,
            disabled: false,
            area: Rect::ZERO,
        }
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    pub fn on_key(&mut self, key: &Key) -> Outcome {
        if self.disabled {
            return Outcome::Ignored;
        }
        if key.is_char(' ') || key.is(KeyCode::Enter) {
            self.on = !self.on;
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    pub fn on_click(&mut self) -> Outcome {
        if self.disabled {
            return Outcome::Consumed;
        }
        self.on = !self.on;
        Outcome::Changed
    }

    /// Natural width of the toggle row, saturating at `u16::MAX`.
    pub fn width(&self) -> u16 {
        terminal_extent(crate::ui::text::width(&self.label).saturating_add(10))
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        let area = area.intersection(*buf.area());
        let area = Rect::new(area.x, area.y, area.width, 1.min(area.height));
        self.area = area;
        if area.is_empty() {
            return;
        }
        let t = ctx.theme;
        let mut s = ctx.state(self.id);
        s.disabled = self.disabled;
        if self.disabled {
            s.hovered = false;
        }
        let st = RowUi::paint(ctx, buf, area, s, RowLook::Bar, bg).style();
        let g = &t.glyphs;
        let (sw, ss) = if self.disabled {
            (if self.on { g.toggle_on } else { g.toggle_off }, st)
        } else if self.on {
            (g.toggle_on, st.fg(t.chosen_in(s)))
        } else {
            (g.toggle_off, st.fg(t.text_muted))
        };
        let sw = if area.width < 4 {
            if self.on { g.dot_on } else { g.dot_off }
        } else {
            sw
        };
        row_part(buf, area, 1, sw, ss);
        row_part(
            buf,
            area,
            5,
            &crate::ui::text::truncate(&self.label, area.width.saturating_sub(5) as usize),
            st,
        );
        let state = if self.on { "on" } else { "off" };
        let state_offset = 6usize.saturating_add(crate::ui::text::width(&self.label));
        if state_offset.saturating_add(3) < usize::from(area.width) {
            row_part(
                buf,
                area,
                state_offset as u16,
                state,
                st.fg(if self.disabled {
                    t.disabled
                } else {
                    t.text_muted
                }),
            );
        }
        ctx.control(self.id, area, self.disabled);
    }
}

impl RowSource for RadioGroup {
    fn len(&self) -> usize {
        self.options.len()
    }
    fn label(&self, i: usize) -> &str {
        &self.options[i]
    }
}

impl Navigable for RadioGroup {
    fn cursor(&self) -> Cursor {
        Cursor::new(self.cursor)
    }
}

impl crate::ui::specimen::Specimen for Checkbox {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: crate::ui::specimen::SpecimenState) -> bool {
        use crate::ui::specimen::SpecimenState::*;
        matches!(
            state,
            Rest | Hover | Pressed | Focused | FocusedHover | Disabled | Overflow
        )
    }

    fn specimen(
        &self,
        state: crate::ui::specimen::SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        let mut c = self.clone();
        c.disabled |= state.visual().disabled;
        if state == crate::ui::specimen::SpecimenState::Overflow {
            c.label = format!("{} with a long label", c.label);
        }
        c.render(area, buf, ctx, bg);
    }
}

impl crate::ui::specimen::Specimen for Toggle {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: crate::ui::specimen::SpecimenState) -> bool {
        use crate::ui::specimen::SpecimenState::*;
        matches!(
            state,
            Rest | Hover | Pressed | Focused | FocusedHover | Disabled | Overflow
        )
    }

    fn specimen(
        &self,
        state: crate::ui::specimen::SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        let mut t = self.clone();
        t.disabled |= state.visual().disabled;
        if state == crate::ui::specimen::SpecimenState::Overflow {
            t.label = format!("{} with a long label", t.label);
        }
        t.render(area, buf, ctx, bg);
    }
}

impl crate::ui::specimen::Specimen for RadioGroup {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: crate::ui::specimen::SpecimenState) -> bool {
        use crate::ui::specimen::SpecimenState::*;
        matches!(state, Rest | Hover | Focused | FocusedHover | Disabled)
    }

    fn specimen_height(&self, _state: crate::ui::specimen::SpecimenState) -> u16 {
        self.height()
    }

    fn specimen(
        &self,
        state: crate::ui::specimen::SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        let mut g = self.clone();
        let v = state.visual();
        g.disabled |= v.disabled;

        if v.hovered {
            ctx.interaction.hover = Some(g.option_id(g.cursor));
        }
        g.render(area, buf, ctx, bg);
    }
}

impl crate::core::pointer::MouseTarget for Checkbox {
    type Action = ();

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn pointer(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> crate::core::response::Response<()> {
        if !ev.is(crate::core::intent::Phase::Click) {
            return crate::core::response::Response::ignored();
        }
        self.on_click().into()
    }
}

impl crate::core::pointer::MouseTarget for RadioGroup {
    type Action = ();

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn pointer(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> crate::core::response::Response<()> {
        match ev.index_of(Part::ROW) {
            Some(i) if ev.is(crate::core::intent::Phase::Click) => self.on_click(i).into(),
            _ => crate::core::response::Response::ignored(),
        }
    }
}

impl crate::core::pointer::MouseTarget for Toggle {
    type Action = ();

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn pointer(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> crate::core::response::Response<()> {
        if !ev.is(crate::core::intent::Phase::Click) {
            return crate::core::response::Response::ignored();
        }
        self.on_click().into()
    }
}
