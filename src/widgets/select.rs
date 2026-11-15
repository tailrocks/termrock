use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Margin, Position, Rect};
use ratatui::style::Color;

use crate::core::event::{Key, Outcome};
use crate::core::id::{Part, WidgetId};
use crate::core::keymap::Nav;
use crate::core::keymap::NavKeys;
use crate::core::keymap::unassigned_chord;
use crate::core::list::{Cursor, KeyStep, Moved, Navigable, RowSource};
use crate::core::refusal::{Control, Reason};
use crate::core::scroll::{Scrollable, TrackClick};
use crate::theme::CursorStyle;
use crate::ui::ctx::{RenderCtx, fill};
use crate::ui::popup::{Dismiss, Placement, backdrop_id, place_with, surface_for};
use crate::widgets::option_list::OptionList;
use crate::widgets::scrollbar;

pub use crate::widgets::option_list::{SelectArrows, WheelMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabInOpen {
    #[default]
    Choose,

    Close,

    Swallow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Press {
    opened: bool,

    label: bool,
    origin: Position,

    moved: bool,
}

#[derive(Debug)]
pub struct Select {
    pub id: WidgetId,
    pub label: String,
    pub options: Vec<String>,
    pub selected: usize,
    pub open: bool,
    pub disabled: bool,
    pub control: Control,
    pub help: String,
    pub area: Rect,

    pub list: OptionList,

    pub reveal: Option<crate::ui::reveal::Reveal>,

    pub arrows: SelectArrows,

    pub tab_in_open: TabInOpen,

    pub mixed: Option<String>,

    pub type_ahead_precedence: Option<crate::core::list::TypeAheadPrecedence>,

    pub press_drag: bool,

    pub label_hit: bool,

    label_area: Rect,
    press: Option<Press>,
}

impl Clone for Select {
    fn clone(&self) -> Self {
        #[cfg(test)]
        crate::core::work::add(crate::core::work::Counter::OptionClones, self.options.len());
        Self {
            id: self.id,
            label: self.label.clone(),
            options: self.options.clone(),
            selected: self.selected,
            open: self.open,
            disabled: self.disabled,
            control: self.control.clone(),
            help: self.help.clone(),
            area: self.area,
            list: self.list.clone(),
            reveal: self.reveal,
            arrows: self.arrows,
            tab_in_open: self.tab_in_open,
            mixed: self.mixed.clone(),
            type_ahead_precedence: self.type_ahead_precedence,
            press_drag: self.press_drag,
            label_hit: self.label_hit,
            label_area: self.label_area,
            press: self.press,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectAction {
    Changed(usize),
}

impl Select {
    pub const HEIGHT: u16 = 3;

    pub const MAX_ROWS: u16 = 8;

    pub const POPUP_WIDTH: (u16, u16) = (12, 40);

    pub fn new(id: WidgetId, label: &str, options: &[&str], selected: usize) -> Self {
        let mut list = OptionList::new(id)
            .max_rows(Self::MAX_ROWS)
            .width(Self::POPUP_WIDTH.0, Self::POPUP_WIDTH.1)
            .clear_on_leave(false)
            .fade_edges(false);
        list.cursor = Cursor::new(selected);

        list.scroll.v.context = 0;
        list.scroll.v.viewport_len = Self::MAX_ROWS as usize;
        Self {
            id,
            label: label.to_owned(),
            options: options.iter().map(|s| (*s).to_owned()).collect(),
            selected,
            open: false,
            disabled: false,
            control: Control::Live,
            help: String::new(),
            area: Rect::ZERO,
            list,
            reveal: None,
            arrows: SelectArrows::Open,
            tab_in_open: TabInOpen::Choose,
            mixed: None,
            type_ahead_precedence: None,
            press_drag: true,
            label_hit: true,
            label_area: Rect::ZERO,
            press: None,
        }
    }

    pub fn wheel(mut self, mode: WheelMode) -> Self {
        self.list.wheel = mode;
        self
    }

    pub fn hover_moves_cursor(mut self, on: bool) -> Self {
        self.list.hover_moves_cursor = on;
        self
    }

    pub fn press_drag(mut self, on: bool) -> Self {
        self.press_drag = on;
        self
    }

    pub fn label_hit(mut self, on: bool) -> Self {
        self.label_hit = on;
        self
    }

    pub fn track_click(mut self, click: TrackClick) -> Self {
        self.list.scroll.track_click = Some(click);
        self
    }

    pub fn bar_id(&self) -> WidgetId {
        scrollbar::id_for(self.id)
    }

    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.list.nav = nav;
        self
    }

    pub fn dismiss(mut self, d: Dismiss) -> Self {
        self.list.dismiss = d;
        self
    }

    pub fn max_rows(mut self, rows: u16) -> Self {
        self.list.max_rows = rows.max(1);
        self
    }

    pub fn placement(mut self, placement: Placement) -> Self {
        self.list.placement = placement;
        self
    }

    pub fn popup_fit(mut self, fit: crate::ui::popup::Place) -> Self {
        self.list.fit = fit;
        self
    }

    pub fn popup_width(mut self, min: u16, max: u16) -> Self {
        let min = min.max(1);
        self.list.width = (min, max.max(min));
        self
    }

    pub fn cursor_style(mut self, cursor_style: CursorStyle) -> Self {
        self.list.look = cursor_style;
        self
    }
    pub fn help(mut self, h: &str) -> Self {
        self.help = h.to_owned();
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    pub fn control(mut self, control: Control) -> Self {
        self.control = control;
        self
    }

    pub fn fixed(self, reason: Reason) -> Self {
        self.control(Control::fixed(reason))
    }

    fn typeahead_precedence(&self) -> crate::core::list::TypeAheadPrecedence {
        if self.open {
            crate::core::list::TypeAheadPrecedence::TypeAhead
        } else {
            self.type_ahead_precedence
                .unwrap_or_else(crate::core::list::TypeAheadPrecedence::current)
        }
    }

    /// Predicts `outcome != Ignored || action.is_some()` for the live path
    /// without copying options, searching labels or changing state.
    fn would_handle_live_key(&self, key: &Key) -> bool {
        let step = KeyStep::ordered(
            key,
            &self.list.nav,
            self.list.type_ahead,
            self.typeahead_precedence(),
        );
        if let Some(KeyStep::Type(_)) = step {
            return !self.options.is_empty();
        }
        if self.open {
            if let Some(KeyStep::Nav(_)) = step {
                return true;
            }
            return match key.code {
                KeyCode::Tab | KeyCode::BackTab => match self.tab_in_open {
                    TabInOpen::Choose => {
                        self.list.cursor.index != self.selected || self.mixed.is_some()
                    }
                    TabInOpen::Close => false,
                    TabInOpen::Swallow => true,
                },
                KeyCode::Char('?') | KeyCode::F(_) => false,
                _ => true,
            };
        }
        if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
            return true;
        }
        let nav = self
            .list
            .nav
            .action(key)
            .filter(|_| !matches!(key.code, KeyCode::Char(_)));
        match nav {
            Some(Nav::Up | Nav::Down) if key.plain() && self.arrows == SelectArrows::Open => true,
            Some(Nav::Left | Nav::Right) if self.arrows == SelectArrows::Open => false,
            Some(Nav::Up | Nav::Left | Nav::Down | Nav::Right) if key.plain() => {
                !self.options.is_empty()
            }
            _ => false,
        }
    }

    pub fn mixed(mut self, placeholder: &str) -> Self {
        self.mixed = Some(placeholder.to_owned());
        self
    }

    pub fn set_mixed(&mut self, placeholder: Option<&str>) {
        self.mixed = placeholder.map(str::to_owned);
    }

    fn choose(&mut self, i: usize) -> Option<SelectAction> {
        let changed = i != self.selected || self.mixed.is_some();
        self.selected = i;
        self.mixed = None;
        changed.then_some(SelectAction::Changed(i))
    }

    pub fn value(&self) -> &str {
        self.options
            .get(self.selected)
            .map(String::as_str)
            .unwrap_or("")
    }
    pub fn option_id(&self, i: usize) -> WidgetId {
        self.list.row_id(i)
    }

    fn option_of(&self, id: WidgetId) -> Option<usize> {
        self.list.row_of(id, self.options.len())
    }

    pub(crate) fn contains_id(&self, id: WidgetId) -> bool {
        id == self.id
            || id == backdrop_id(self.id)
            || id == self.bar_id()
            || self.option_of(id).is_some()
    }

    fn value_cursor(&self) -> Cursor {
        Cursor::new(self.selected).wrap(self.list.cursor.wrap)
    }

    fn open_list(&mut self) {
        self.open = true;
        self.list.cursor.index = self.selected;

        let visible = self.list.scroll.viewport_len.max(1);
        self.list.scroll.offset = self.selected.saturating_sub(visible - 1);
    }

    fn shut(&mut self) {
        self.open = false;
        self.list.cursor.index = self.selected;
    }

    pub fn on_key(&mut self, key: &Key) -> (Outcome, Option<SelectAction>) {
        if self.disabled {
            return (Outcome::Ignored, None);
        }
        if unassigned_chord::<()>(key, &[]) {
            return (Outcome::Ignored, None);
        }
        if self.control.is_fixed() {
            if self.would_handle_live_key(key) {
                self.control.refuse();
                return (Outcome::Consumed, None);
            }
            return (Outcome::Ignored, None);
        }

        let step = KeyStep::ordered(
            key,
            &self.list.nav,
            self.list.type_ahead,
            self.typeahead_precedence(),
        );
        if let Some(KeyStep::Type(c)) = step {
            if !self.open {
                self.list.cursor.index = self.selected;
            }
            let moved = self.list.type_char(c, &self.options[..]);
            if let Moved::To(i) = moved
                && !self.open
            {
                return (Outcome::Changed, self.choose(i));
            }
            return (moved.outcome(), None);
        }
        if self.open {
            if let Some(KeyStep::Nav(nav)) = step {
                return match self.list.nav_step(nav, &self.options[..]).index() {
                    Some(_) => (Outcome::Changed, None),

                    None => (Outcome::Consumed, None),
                };
            }
            return match key.code {
                KeyCode::Enter | KeyCode::Char(' ') => {
                    self.open = false;
                    (Outcome::Changed, self.choose(self.list.cursor.index))
                }
                KeyCode::Esc if self.list.dismiss.escape => {
                    self.shut();
                    (Outcome::Changed, None)
                }

                KeyCode::Tab | KeyCode::BackTab => match self.tab_in_open {
                    TabInOpen::Choose => {
                        self.open = false;
                        (Outcome::Ignored, self.choose(self.list.cursor.index))
                    }
                    TabInOpen::Close => {
                        self.shut();
                        (Outcome::Ignored, None)
                    }
                    TabInOpen::Swallow => (Outcome::Consumed, None),
                },

                KeyCode::Char('?') | KeyCode::F(_) => {
                    self.shut();
                    (Outcome::Ignored, None)
                }
                _ => (Outcome::Consumed, None),
            };
        }
        if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
            self.open_list();
            return (Outcome::Changed, None);
        }

        let nav = self
            .list
            .nav
            .action(key)
            .filter(|_| !matches!(key.code, KeyCode::Char(_)));
        match nav {
            Some(Nav::Up | Nav::Down) if key.plain() && self.arrows == SelectArrows::Open => {
                self.open_list();
                (Outcome::Changed, None)
            }
            Some(Nav::Left | Nav::Right) if self.arrows == SelectArrows::Open => {
                (Outcome::Ignored, None)
            }
            Some(nav @ (Nav::Up | Nav::Left | Nav::Down | Nav::Right)) if key.plain() => {
                let delta = if matches!(nav, Nav::Up | Nav::Left) {
                    -1
                } else {
                    1
                };
                match self.value_cursor().step(delta, &self.options[..]).index() {
                    Some(n) => {
                        self.list.cursor.index = n;
                        (Outcome::Changed, self.choose(n))
                    }
                    None => (Outcome::Ignored, None),
                }
            }
            _ => (Outcome::Ignored, None),
        }
    }

    pub fn wrap(mut self, on: bool) -> Self {
        self.list.cursor.wrap = on;
        self
    }

    pub fn type_ahead_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.list = self.list.type_ahead_timeout(timeout);
        self
    }

    pub fn type_ahead(mut self, on: bool) -> Self {
        self.list.type_ahead = on;
        self
    }

    pub fn type_ahead_precedence(
        mut self,
        precedence: crate::core::list::TypeAheadPrecedence,
    ) -> Self {
        self.type_ahead_precedence = Some(precedence);
        self
    }

    pub fn tab_in_open(mut self, tab: TabInOpen) -> Self {
        self.tab_in_open = tab;
        self
    }

    pub fn arrows(mut self, arrows: SelectArrows) -> Self {
        self.arrows = arrows;
        self
    }

    pub fn on_click(&mut self, id: WidgetId) -> (Outcome, Option<SelectAction>) {
        if self.disabled {
            return (Outcome::Consumed, None);
        }
        if id == self.bar_id() {
            self.list.scroll.release();
            return (Outcome::Consumed, None);
        }
        if id == self.id && self.control.refuse() {
            return (Outcome::Consumed, None);
        }
        if id == self.id {
            if let Some(p) = self.press.take()
                && (p.opened || p.label)
            {
                return (Outcome::Changed, None);
            }
            if self.open {
                self.shut();
            } else {
                self.open_list();
            }
            return (Outcome::Changed, None);
        }
        if id == backdrop_id(self.id) {
            self.press = None;
            return (self.on_click_outside(), None);
        }
        if let Some(i) = self.option_of(id) {
            self.open = false;
            return (Outcome::Changed, self.choose(i));
        }
        (Outcome::Ignored, None)
    }

    pub fn close(&mut self) -> Outcome {
        if self.open {
            self.shut();
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    pub fn close_on_blur(mut self, on: bool) -> Self {
        self.list.dismiss.focus_out = on;
        self
    }

    pub fn on_blur(&mut self) -> Outcome {
        if self.list.dismiss.focus_out {
            self.close()
        } else {
            Outcome::Ignored
        }
    }

    pub fn on_click_outside(&mut self) -> Outcome {
        if !self.open {
            return Outcome::Ignored;
        }
        if self.list.dismiss.outside_click {
            self.close()
        } else {
            Outcome::Consumed
        }
    }

    pub fn on_press(&mut self, id: WidgetId, pos: Position) -> Outcome {
        if self.disabled {
            return Outcome::Consumed;
        }
        if id == self.bar_id() {
            if !self.open || self.list.scroll.v_track().is_empty() {
                return Outcome::Ignored;
            }
            return self.list.scroll.press(pos).outcome();
        }
        if id != self.id {
            return Outcome::Ignored;
        }
        if self.control.refuse() {
            return Outcome::Consumed;
        }
        let label = self.label_hit && self.label_area.contains(pos);
        let opened = !label && self.press_drag && !self.open;
        if opened {
            self.open_list();
        }
        self.press = Some(Press {
            opened,
            label,
            origin: pos,
            moved: false,
        });
        if opened {
            Outcome::Changed
        } else {
            Outcome::Consumed
        }
    }

    pub fn on_drag(&mut self, pressed: WidgetId, pos: Position) -> Outcome {
        if self.disabled || !self.open {
            return Outcome::Ignored;
        }
        if pressed == self.bar_id() {
            if self.list.scroll.v_track().is_empty() {
                return Outcome::Ignored;
            }
            return self.list.scroll.drag(pos).outcome();
        }
        if pressed != self.id || !self.press_drag {
            return Outcome::Ignored;
        }
        let row = self.list.row_at(pos);
        let Some(p) = self.press.as_mut() else {
            return Outcome::Ignored;
        };
        p.moved |= pos != p.origin;
        if let Some(i) = row {
            self.list.cursor.index = i;
        }
        Outcome::Changed
    }

    pub fn on_release(&mut self, pos: Position) -> (Outcome, Option<SelectAction>) {
        self.list.scroll.release();
        let Some(p) = self.press.take() else {
            return (Outcome::Ignored, None);
        };
        if self.disabled || !self.open || !p.moved || !self.press_drag {
            return (Outcome::Consumed, None);
        }
        if let Some(i) = self.list.row_at(pos) {
            self.open = false;
            return (Outcome::Changed, self.choose(i));
        }
        if self.area.contains(pos) {
            return (Outcome::Consumed, None);
        }
        (self.close(), None)
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            return;
        }
        let t = ctx.theme;
        self.list.scroll.begin(ctx.track_click);
        let mut s = ctx.state(self.id);
        s.fixed = self.control.is_fixed();
        s.disabled = self.disabled || s.fixed;
        if s.disabled {
            s.hovered = false;
            self.open = false;
        }

        let label_style = if s.disabled {
            t.faint().bg(bg)
        } else {
            t.label(s.focused).bg(bg)
        };
        buf.set_string(area.x + 2, area.y, &self.label, label_style);
        self.label_area = Rect::ZERO;
        if self.label_hit && !self.label.is_empty() {
            let lw = crate::ui::text::width(&self.label).min(area.width.saturating_sub(2) as usize);
            self.label_area = Rect::new(area.x, area.y, 2 + lw as u16, 1);
            ctx.clickable(self.id, self.label_area);
        }
        if area.height < 2 {
            return;
        }
        let field = Rect::new(area.x, area.y + 1, area.width, 1);
        self.area = field;
        let fs = t.field_style(s);
        fill(buf, field, fs);
        buf.set_string(
            field.x,
            field.y,
            t.gutter_symbol(s),
            t.gutter(s, fs.bg.unwrap_or(bg), false),
        );
        let shown = self.mixed.as_deref().unwrap_or(self.value());
        let value = crate::ui::text::truncate(shown, field.width.saturating_sub(5) as usize);
        let vs = if self.mixed.is_some() {
            fs.patch(t.placeholder(s))
        } else {
            fs
        };
        buf.set_string(field.x + 2, field.y, &value, vs);
        if !self.disabled && !self.open && self.mixed.is_none() {
            let cut = Rect::new(field.x + 2, field.y, field.width.saturating_sub(5), 1);
            crate::ui::reveal::truncated(ctx, self.reveal, self.id, cut, self.value());
        }
        buf.set_string(
            field.right().saturating_sub(2),
            field.y,
            if self.open {
                t.glyphs.chevron_open
            } else {
                t.glyphs.chevron_closed
            },
            fs.fg(if s.disabled {
                t.disabled
            } else {
                t.text_secondary
            }),
        );
        ctx.control(self.id, field, self.disabled);
        if area.height >= 3 && !self.help.is_empty() {
            buf.set_string(
                area.x + 2,
                area.y + 2,
                crate::ui::text::truncate(&self.help, area.width.saturating_sub(2) as usize),
                t.muted().bg(bg),
            );
        }
        if self.open {
            self.queue_list(field, buf, ctx);
        }
    }

    fn queue_list(&mut self, field: Rect, buf: &Buffer, ctx: &mut RenderCtx) {
        let screen = *buf.area();
        let rows = (self.options.len() as u16).clamp(1, self.list.max_rows);
        let (min_w, max_w) = self.list.width;
        let w = field.width.clamp(min_w, max_w);
        let pa = place_with(
            screen,
            field,
            w,
            rows + 2,
            self.list.placement,
            self.list.fit,
        );
        let inner = pa.intersection(screen).inner(Margin::new(1, 1));

        self.list.layout(inner, &self.options[..], ctx);

        self.list.chosen = self.mixed.is_none().then_some(self.selected);
        let options = &self.options;
        let snapshot = self
            .list
            .snapshot(inner, &options[..], |i| options[i].clone());
        let (id, dismiss, reveal) = (self.id, self.list.dismiss, self.reveal);
        ctx.overlay(move |buf, ctx| {
            let t = ctx.theme;
            let inner = surface_for(pa, buf, ctx, t, Some((id, dismiss)));

            ctx.scrollable(id, inner);
            snapshot.paint_rows(
                inner,
                t.surface_elevated,
                buf,
                ctx,
                |row, paint, buf, ctx| {
                    let opt = &row.payload;
                    let area = paint.area;
                    let st = paint.style();
                    if paint.state.selected {
                        let ink = st.fg(paint.ink(ctx, t.text_secondary));
                        buf.set_string(area.x + 1, area.y, t.glyphs.current, ink);
                    }
                    buf.set_string(
                        area.x + 3,
                        area.y,
                        crate::ui::text::truncate(opt, area.width.saturating_sub(3) as usize),
                        st,
                    );
                    let cut = Rect::new(area.x + 3, area.y, area.width.saturating_sub(3), 1);
                    crate::ui::reveal::truncated(ctx, reveal, snapshot.row_id(row.index), cut, opt);
                },
            );

            let edge = t.text_secondary;
            let x = pa.right().saturating_sub(3);
            if snapshot.more_above() {
                buf.set_string(
                    x,
                    pa.y,
                    t.glyphs.more_above,
                    ratatui::style::Style::new().fg(edge).bg(t.surface_elevated),
                );
            }
            if snapshot.more_below() {
                buf.set_string(
                    x,
                    pa.bottom().saturating_sub(1),
                    t.glyphs.more_below,
                    ratatui::style::Style::new().fg(edge).bg(t.surface_elevated),
                );
            }
        });
    }
}

impl crate::core::focus::BlurTarget for Select {
    fn blur_id(&self) -> WidgetId {
        self.id
    }
    fn blur(&mut self) -> Outcome {
        self.on_blur()
    }
}

impl RowSource for Select {
    fn len(&self) -> usize {
        self.options.len()
    }
    fn label(&self, i: usize) -> &str {
        <[String] as RowSource>::label(&self.options, i)
    }
}

impl Navigable for Select {
    fn cursor(&self) -> Cursor {
        if self.open {
            self.list.cursor
        } else {
            self.value_cursor()
        }
    }
}

impl crate::ui::specimen::Specimen for Select {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: crate::ui::specimen::SpecimenState) -> bool {
        use crate::ui::specimen::SpecimenState::*;
        match state {
            Rest | Hover | Pressed | Focused | FocusedHover | Disabled | Empty | Overflow
            | Narrow => true,
            Selected | Busy | Error | Editing | Loading => false,
        }
    }

    fn specimen_height(&self, _state: crate::ui::specimen::SpecimenState) -> u16 {
        2
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
        let mut s = self.clone();
        s.open = false;
        s.help.clear();
        s.disabled |= state.visual().disabled;
        match state {
            SpecimenState::Empty => s.set_mixed(Some("Mixed")),
            SpecimenState::Overflow => {
                if let Some(o) = s.options.get_mut(s.selected) {
                    *o = format!("{o} and a value longer than the field");
                }
            }
            _ => {}
        }
        s.render(area, buf, ctx, bg);
    }
}

impl crate::core::pointer::MouseTarget for Select {
    type Action = SelectAction;

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn pointer(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> crate::core::response::Response<SelectAction> {
        use crate::core::intent::Phase;
        use crate::core::response::Response;
        if let Some((axis, delta)) = ev.wheel() {
            if ev.part().part == Part::BACKDROP || !self.open || self.disabled {
                return Response::ignored();
            }
            return self.list.wheel(axis, delta).lift();
        }
        let (id, pos) = (ev.id, ev.pos());
        match ev.phase() {
            Some(Phase::Press) => self.on_press(id, pos).into(),
            Some(Phase::Drag) => self.on_drag(id, pos).into(),
            Some(Phase::Release) => self.on_release(pos).into(),
            Some(Phase::Click) => self.on_click(id).into(),
            _ => Response::ignored(),
        }
    }
}
