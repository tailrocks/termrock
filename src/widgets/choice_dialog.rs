use crate::core::event::{Key, Outcome};
use crate::core::focus::{Focus, FocusRing};
use crate::core::hints::{HasHints, HintCx};
use crate::core::id::WidgetId;
use crate::core::intent::DismissReason;
use crate::core::keymap::{Nav, NavKeys};
use crate::theme::{ButtonKind, Strings, Tone};
use crate::ui::ctx::RenderCtx;
use crate::ui::text::truncate;
use crate::widgets::button::{Button, row_layout_right};
use crate::widgets::choice::RadioGroup;
use crate::widgets::keyhint::Hint;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::Style;

#[derive(Clone)]
pub struct ChoiceDialog {
    pub close_button: crate::widgets::icon_button::IconButton,

    pub dismiss: crate::ui::popup::Dismiss,
    pub title: String,
    pub lines: Vec<(String, Tone)>,
    pub radio: RadioGroup,
    pub buttons: Vec<Button>,

    pub cancel_index: usize,

    pub result: Option<Option<usize>>,

    pub fired: Option<usize>,
    pub width: u16,
    pub stepper: Option<String>,
    pub area: Rect,
    pub option_tones: Vec<Tone>,

    strings: Strings,

    pub nav: NavKeys,

    pub button_row: crate::widgets::button::ButtonRow,

    pub double_click_confirms: bool,

    refused: bool,
}

impl HasHints for ChoiceDialog {
    fn hints(&self, cx: &HintCx) -> Vec<Hint> {
        let mut v = vec![];
        if cx.focus == Some(self.radio.id) || cx.focus.is_none() {
            if let Some(k) = self.nav.move_hint(false) {
                v.push(Hint::new(k, self.strings.hint_choose));
            }
            let primary = self
                .buttons
                .iter()
                .find(|b| b.kind == ButtonKind::Primary || b.kind == ButtonKind::Danger)
                .or(self.buttons.last());
            if let Some(b) = primary {
                v.push(b.hint());
            }
        } else if let Some(b) = self.buttons.iter().find(|b| cx.focus == Some(b.id)) {
            v.push(b.hint());
        }
        v.push(Hint::new("Tab", self.strings.hint_next));
        if self.dismiss.escape
            && let Some(b) = self.buttons.get(self.cancel_index)
        {
            v.push(Hint::new("Esc", b.label.clone()));
        }
        v
    }
}

impl ChoiceDialog {
    pub fn button_row(mut self, row: crate::widgets::button::ButtonRow) -> Self {
        self.button_row = row;
        self
    }

    pub fn double_click_confirms(mut self, on: bool) -> Self {
        self.double_click_confirms = on;
        self
    }

    pub fn close_button(mut self, button: crate::widgets::icon_button::IconButton) -> Self {
        self.close_button = button;
        self
    }

    pub fn dismiss(mut self, dismiss: crate::ui::popup::Dismiss) -> Self {
        self.dismiss = dismiss;
        self
    }

    pub fn new(id: WidgetId, title: &str, label: &str, options: &[&str], selected: usize) -> Self {
        Self {
            dismiss: crate::ui::popup::Dismiss::ESCAPE,
            close_button: crate::widgets::icon_button::IconButton::close(crate::ui::modal::CLOSE),
            title: title.to_owned(),
            lines: vec![],
            radio: RadioGroup::new(id.sub("radio"), label, options, selected),
            buttons: vec![
                Button::subtle(id.sub("cancel"), Strings::DEFAULT.cancel),
                Button::primary(id.sub("ok"), Strings::DEFAULT.next),
            ],
            cancel_index: 0,
            result: None,
            fired: None,
            width: 54,
            stepper: None,
            area: Rect::ZERO,
            option_tones: vec![],
            strings: Strings::DEFAULT,
            nav: NavKeys::default(),
            button_row: crate::widgets::button::ButtonRow::new(),
            double_click_confirms: true,
            refused: false,
        }
    }

    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.nav = nav;
        self
    }

    pub fn buttons(mut self, buttons: Vec<Button>, cancel_index: usize) -> Self {
        self.buttons = buttons;
        self.cancel_index = cancel_index;
        self
    }

    pub fn line(mut self, text: impl Into<String>, tone: Tone) -> Self {
        self.lines.push((text.into(), tone));
        self
    }

    pub fn stepper(mut self, s: &str) -> Self {
        self.stepper = Some(s.to_owned());
        self
    }

    pub fn width(mut self, w: u16) -> Self {
        self.width = w;
        self
    }

    pub fn option_tones(mut self, tones: Vec<Tone>) -> Self {
        self.option_tones = tones;
        self
    }

    pub fn initial_focus(&self) -> WidgetId {
        if self.radio.disabled {
            return self
                .buttons
                .get(self.primary())
                .map_or(self.radio.id, |b| b.id);
        }
        self.radio.id
    }

    pub fn height(&self) -> u16 {
        let body = self.lines.len() as u16 + if self.lines.is_empty() { 0 } else { 1 };
        let stepper = if self.stepper.is_some() { 2 } else { 0 };
        2 + 2 + stepper + body + self.radio.height() + 2 + 1
    }

    fn primary(&self) -> usize {
        self.buttons
            .iter()
            .position(|b| b.kind == ButtonKind::Primary || b.kind == ButtonKind::Danger)
            .unwrap_or(self.buttons.len().saturating_sub(1))
    }

    fn fire_default(&mut self) -> Outcome {
        let i = self.primary();
        if self.buttons.get(i).is_some_and(Button::can_activate) {
            self.fire(i)
        } else {
            self.refused = true;
            Outcome::Changed
        }
    }

    pub(crate) fn cancel_implicit(&mut self) {
        self.fired = Some(self.cancel_index);
        self.result = Some(None);
    }

    fn fire(&mut self, i: usize) -> Outcome {
        self.fired = Some(i);
        self.result = Some(if i == self.cancel_index {
            None
        } else {
            Some(self.radio.selected)
        });
        Outcome::Changed
    }

    pub fn on_key(&mut self, key: &Key, focus: &mut Focus, ring: &FocusRing) -> Outcome {
        self.refused = false;
        let cur = focus.current();
        if cur == Some(self.radio.id) {
            if key.is(KeyCode::Enter) {
                return self.fire_default();
            }
            let o = self.radio.on_key(key);
            if o.consumed() {
                return o;
            }
        }
        for i in 0..self.buttons.len() {
            if cur == Some(self.buttons[i].id) {
                let (o, fired) = self.buttons[i].on_key(key);
                if fired {
                    return self.fire(i);
                }
                if o.consumed() {
                    return o;
                }
            }
        }
        let step = match self.nav.action(key) {
            Some(Nav::Left) => Some(true),
            Some(Nav::Right) => Some(false),
            _ => None,
        };
        if let Some(back) = step {
            if let Some(id) = self.button_row.step(&self.buttons, cur, !back) {
                focus.focus(id);
            }
            return Outcome::Changed;
        }
        match key.code {
            KeyCode::Esc => crate::ui::layer::dismiss(self, DismissReason::Esc)
                .map_or(Outcome::Consumed, |response| response.outcome()),
            KeyCode::Tab => {
                focus.next(ring);
                Outcome::Changed
            }
            KeyCode::BackTab => {
                focus.prev(ring);
                Outcome::Changed
            }
            _ => Outcome::Consumed,
        }
    }

    pub fn on_click(&mut self, id: WidgetId, focus: &mut Focus) -> Outcome {
        for i in 0..self.radio.options.len() {
            if self.radio.option_id(i) == id {
                if !self.radio.disabled {
                    focus.focus(self.radio.id);
                }
                return self.radio.on_click(i);
            }
        }
        for i in 0..self.buttons.len() {
            if self.buttons[i].id == id {
                focus.focus(id);
                if self.buttons[i].on_click() {
                    return self.fire(i);
                }
                return Outcome::Changed;
            }
        }
        Outcome::Consumed
    }

    pub fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let s = ctx.theme.strings;
        let old = std::mem::replace(&mut self.strings, s);
        for (i, b) in self.buttons.iter_mut().enumerate() {
            if i == self.cancel_index {
                Strings::follow(&mut b.label, old.cancel, s.cancel);
            } else {
                Strings::follow(&mut b.label, old.next, s.next);
            }
        }
        let h = self.height();

        let flash = ctx.interaction.flash;
        if self.refused {
            ctx.interaction.flash = Some(crate::ui::modal::SURFACE);
        }
        let (area, inner) = crate::ui::modal::Frame::new(self.width, h)
            .title(&self.title)
            .meta(None)
            .overlay(&*self)
            .render(screen, buf, ctx);
        ctx.interaction.flash = flash;
        self.area = area;
        let t = ctx.theme;
        let bg = t.surface_elevated;
        let mut y = inner.y;
        if let Some(s) = &self.stepper {
            buf.set_string(
                inner.x,
                y,
                truncate(s, inner.width as usize),
                t.muted().bg(bg),
            );
            y += 2;
        }
        for (l, tone) in &self.lines {
            buf.set_string(
                inner.x,
                y,
                truncate(l, inner.width as usize),
                Style::new().fg(t.tone(*tone)).bg(bg),
            );
            y += 1;
        }
        if !self.lines.is_empty() {
            y += 1;
        }
        self.radio.render(
            Rect::new(inner.x - 1, y, inner.width + 1, self.radio.height()),
            buf,
            ctx,
            bg,
        );

        for (i, tone) in self.option_tones.iter().enumerate() {
            if let Some(r) = self.radio.areas.get(i)
                && *tone != Tone::Normal
                && i != self.radio.cursor
            {
                buf.set_string(
                    r.x + 5,
                    r.y,
                    &self.radio.options[i],
                    Style::new().fg(t.tone(*tone)).bg(bg),
                );
            }
        }
        let ay = inner.bottom().saturating_sub(1);
        let widths: Vec<u16> = self.buttons.iter().map(|b| b.width()).collect();
        let rects = row_layout_right(Rect::new(inner.x, ay, inner.width, 1), &widths, 1);
        for (b, r) in self.buttons.iter_mut().zip(rects) {
            b.render(r, buf, ctx, bg);
        }
        for i in 0..self.radio.options.len() {
            if let Some(r) = self.radio.areas.get(i) {
                let part = crate::widgets::choice::RadioGroup::option_part(i);
                let id = self.radio.option_id(i);
                ctx.hits.register_part_as(id, self.radio.id, part, *r);
            }
        }
        for b in &self.buttons {
            ctx.hits.register(b.id, b.area);
        }
    }
}

impl ChoiceDialog {
    pub fn pointer_in(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
        focus: &mut Focus,
    ) -> crate::core::response::Response<()> {
        use crate::core::intent::Phase;
        use crate::core::pointer::MouseTarget;
        use crate::core::response::Response;
        if ev.wheel().is_some() {
            return Response::ignored();
        }
        if ev.is(Phase::Press) {
            self.refused = false;
        }
        if let Some(r) = self.radio.route(ev) {
            if ev.is(Phase::Click) {
                if !self.radio.disabled {
                    focus.focus(self.radio.id);
                }

                if self.double_click_confirms
                    && ev.count() >= 2
                    && !self.radio.disabled
                    && !self.buttons.is_empty()
                    && ev
                        .index_of(crate::core::id::Part::ROW)
                        .is_some_and(|i| i == self.radio.selected)
                {
                    return self.fire_default().into();
                }
            }
            return r;
        }
        if ev.is(Phase::Click) {
            return self.on_click(ev.id, focus).into();
        }
        Response::ignored()
    }
}
