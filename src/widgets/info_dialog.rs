use crate::core::event::{Key, Outcome};
use crate::core::focus::{Focus, FocusRing};
use crate::core::hints::{HasHints, HintCx, nav_hints};
use crate::core::hit::Axis;
use crate::core::id::WidgetId;
use crate::core::keymap::{Actions, KeyBinding, KeyMap, Nav, NavKeys, lookup};
use crate::core::response::Response;
use crate::core::scroll::ScrollPane;
use crate::core::scroll::Scrollable;
use crate::theme::Tone;
use crate::ui::ctx::RenderCtx;
use crate::ui::text::truncate;
use crate::widgets::button::{Button, row_layout_right};
use crate::widgets::keyhint::Hint;
use crate::widgets::props::{Prop, PropsAction, PropsList};
use crate::widgets::scrollbar;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InfoResult {
    Closed,
    Copy(String),

    Action(usize),
}

pub struct InfoDialog {
    pub close_button: crate::widgets::icon_button::IconButton,

    pub dismiss: crate::ui::popup::Dismiss,
    pub id: WidgetId,
    pub title: String,
    pub meta: Option<String>,
    pub intro: Vec<(String, Tone)>,
    pub props: PropsList,
    pub detail: Vec<String>,
    pub detail_scroll: ScrollPane,
    pub actions: Vec<Button>,
    pub close: Button,
    pub width: u16,
    pub result: Option<InfoResult>,
    pub area: Rect,

    pub copy_values: Vec<Option<String>>,
    pub error_title: bool,

    pub nav: NavKeys,

    pub keymap: KeyMap<InfoCmd>,

    strings: crate::theme::Strings,
}

impl HasHints for InfoDialog {
    fn hints(&self, _: &HintCx) -> Vec<Hint> {
        let mut v = nav_hints(&self.nav, false, false);
        if let Some((k, a)) = self.props.reveal.hint() {
            v.push(Hint::new(k, a));
        }
        if self.copy_values.iter().any(Option::is_some)
            && let Some(k) = crate::core::keymap::hint_key(&self.keymap, InfoCmd::Copy)
        {
            v.push(Hint::new(k, "Copy"));
        }
        if self.dismiss.escape {
            v.push(Hint::new("Esc", self.strings.hint_close));
        }
        v
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoCmd {
    Close,

    Copy,
}

impl Actions for InfoCmd {
    fn label(self) -> &'static str {
        match self {
            InfoCmd::Close => "Close",
            InfoCmd::Copy => "Copy",
        }
    }

    fn default_keymap() -> KeyMap<Self> {
        vec![
            (KeyBinding::char('q'), InfoCmd::Close),
            (KeyBinding::char('y'), InfoCmd::Copy),
        ]
    }
}

impl InfoDialog {
    pub fn close_button(mut self, button: crate::widgets::icon_button::IconButton) -> Self {
        self.close_button = button;
        self
    }

    pub fn new(id: WidgetId, title: &str, props: Vec<Prop>) -> Self {
        let copy_values = props
            .iter()
            .map(|p| p.copyable.then(|| p.value.clone()))
            .collect();
        Self {
            dismiss: crate::ui::popup::Dismiss::LIGHT,
            close_button: crate::widgets::icon_button::IconButton::close(crate::ui::modal::CLOSE),
            id,
            title: title.to_owned(),
            meta: None,
            intro: vec![],
            props: PropsList::new(id.sub("props"), props).wrapping(),
            detail: vec![],
            detail_scroll: ScrollPane::default(),
            actions: vec![],
            close: Button::secondary(id.sub("close"), crate::theme::Strings::DEFAULT.close),
            strings: crate::theme::Strings::DEFAULT,
            width: 66,
            result: None,
            area: Rect::ZERO,
            copy_values,
            error_title: false,
            nav: NavKeys::default(),
            keymap: InfoCmd::default_keymap(),
        }
    }

    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.nav = nav;
        self
    }

    pub fn keymap(mut self, keymap: KeyMap<InfoCmd>) -> Self {
        self.keymap = keymap;
        self
    }

    pub fn intro(mut self, lines: Vec<(String, Tone)>) -> Self {
        self.intro = lines;
        self
    }

    pub fn detail(mut self, lines: Vec<String>) -> Self {
        self.detail = lines;
        self
    }

    pub fn action(mut self, b: Button) -> Self {
        self.actions.push(b);
        self
    }

    pub fn width(mut self, w: u16) -> Self {
        self.width = w;
        self
    }

    pub fn meta(mut self, m: &str) -> Self {
        self.meta = Some(m.to_owned());
        self
    }

    pub fn error(mut self) -> Self {
        self.error_title = true;
        self
    }

    /// The intro as drawn in `width` columns: a line that does not fit wraps
    /// onto more lines of its tone.
    fn intro_lines(&self, width: usize) -> Vec<(String, Tone)> {
        self.intro
            .iter()
            .flat_map(|(line, tone)| {
                if crate::ui::text::width(line) <= width {
                    vec![(line.clone(), *tone)]
                } else {
                    crate::ui::text::wrap(line, width)
                        .into_iter()
                        .map(|part| (part, *tone))
                        .collect()
                }
            })
            .collect()
    }

    pub fn initial_focus(&self) -> WidgetId {
        if self.props.props.is_empty() {
            self.close.id
        } else {
            self.props.id
        }
    }

    pub fn on_key(&mut self, key: &Key, focus: &mut Focus, ring: &FocusRing) -> Outcome {
        let cur = focus.current();
        if cur == Some(self.props.id) {
            let (o, ev) = self.props.on_key(key);
            match ev {
                Some(PropsAction::Copy(i)) => {
                    if let Some(Some(v)) = self.copy_values.get(i) {
                        self.result = Some(InfoResult::Copy(v.clone()));
                    }
                    return Outcome::Changed;
                }
                Some(PropsAction::Activate(i)) => {
                    if let Some(Some(v)) = self.copy_values.get(i) {
                        self.result = Some(InfoResult::Copy(v.clone()));
                        return Outcome::Changed;
                    }
                }
                None => {}
            }
            if o.consumed() {
                return o;
            }

            match key.code {
                KeyCode::PageDown => {
                    self.detail_scroll.page_down();
                    return Outcome::Changed;
                }
                KeyCode::PageUp => {
                    self.detail_scroll.page_up();
                    return Outcome::Changed;
                }
                _ => {}
            }
        }
        for i in 0..self.actions.len() {
            if cur == Some(self.actions[i].id) {
                let (o, fired) = self.actions[i].on_key(key);
                if fired {
                    self.result = Some(InfoResult::Action(i));
                    return Outcome::Changed;
                }
                if o.consumed() {
                    return o;
                }
            }
        }
        if cur == Some(self.close.id) {
            let (o, fired) = self.close.on_key(key);
            if fired {
                self.result = Some(InfoResult::Closed);
                return Outcome::Changed;
            }
            if o.consumed() {
                return o;
            }
        }
        match lookup(&self.keymap, key) {
            Some(InfoCmd::Close) => {
                self.result = Some(InfoResult::Closed);
                return Outcome::Changed;
            }
            Some(InfoCmd::Copy) => {
                if let Some(Some(v)) = self.copy_values.iter().find(|v| v.is_some()) {
                    self.result = Some(InfoResult::Copy(v.clone()));
                }
                return Outcome::Changed;
            }
            None => {}
        }
        match self.nav.action(key) {
            Some(Nav::Down) => {
                self.detail_scroll.scroll_by(1);
                return Outcome::Changed;
            }
            Some(Nav::Up) => {
                self.detail_scroll.scroll_by(-1);
                return Outcome::Changed;
            }
            _ => {}
        }
        match key.code {
            KeyCode::Esc => {
                self.result = Some(InfoResult::Closed);
                Outcome::Changed
            }
            KeyCode::Enter => {
                self.result = Some(InfoResult::Closed);
                Outcome::Changed
            }
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

    pub fn on_click(&mut self, id: WidgetId, pos: Position, focus: &mut Focus) -> Outcome {
        if let Some(i) = self.props.row_of(id) {
            focus.focus(self.props.id);

            let (o, act) = self.props.on_click(i);
            if act.is_some()
                && let Some(Some(v)) = self.copy_values.get(i)
            {
                self.result = Some(InfoResult::Copy(v.clone()));
            }
            return o;
        }
        if id == scrollbar::id_for(self.props.id) {
            return self.props.bar_press(pos).outcome().or(Outcome::Consumed);
        }
        if id == scrollbar::id_for(self.id) {
            return self.bar_press(pos).outcome().or(Outcome::Consumed);
        }
        for i in 0..self.actions.len() {
            if self.actions[i].id == id {
                focus.focus(id);
                if self.actions[i].on_click() {
                    self.result = Some(InfoResult::Action(i));
                }
                return Outcome::Changed;
            }
        }
        if id == self.close.id {
            focus.focus(id);
            if self.close.on_click() {
                self.result = Some(InfoResult::Closed);
            }
            return Outcome::Changed;
        }
        Outcome::Consumed
    }

    pub fn on_drag(&mut self, pressed: WidgetId, pos: Position) -> Outcome {
        if scrollbar::is_bar_of(pressed, self.props.id) {
            return match self.props.pane().bar_of(pressed) {
                Some(bar) => self.props.bar_input(&bar, pos, false).outcome(),
                None => Outcome::Consumed,
            };
        }
        if scrollbar::is_bar_of(pressed, self.id) {
            return match self.pane().bar_of(pressed) {
                Some(bar) => self.bar_input(&bar, pos, false).outcome(),
                None => Outcome::Consumed,
            };
        }
        Outcome::Ignored
    }

    pub fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        self.detail_scroll.begin(ctx.track_click);
        let s = ctx.theme.strings;
        let old = std::mem::replace(&mut self.strings, s);
        crate::theme::Strings::follow(&mut self.close.label, old.close, s.close);
        let probe = crate::ui::modal::Frame::new(self.width, 1);
        let inner_w = probe.area(screen).inner(probe.margin).width;
        let intro = self.intro_lines(inner_w as usize);
        let intro_h = intro.len() as u16 + u16::from(!intro.is_empty());
        let props_h = self.props.height_at(inner_w + 1);
        let detail_h = if self.detail.is_empty() {
            0
        } else {
            (self.detail.len() as u16).min(8) + 1
        };
        let h = 2 + 2 + intro_h + props_h + detail_h + 2;
        let title = if self.error_title {
            format!("! {}", self.title)
        } else {
            self.title.clone()
        };
        let (area, inner) = crate::ui::modal::Frame::new(self.width, h)
            .title(&title)
            .meta(self.meta.as_deref())
            .overlay(&*self)
            .render(screen, buf, ctx);
        self.area = area;
        let t = ctx.theme;
        let bg = t.surface_elevated;
        if self.error_title {
            buf.set_string(
                area.x + 3,
                area.y,
                "!",
                t.error_fg().bg(bg).add_modifier(Modifier::BOLD),
            );
        }
        let mut y = inner.y;
        for (l, tone) in &intro {
            if y >= inner.bottom() {
                break;
            }
            buf.set_string(
                inner.x,
                y,
                truncate(l, inner.width as usize),
                Style::new().fg(t.tone(*tone)).bg(bg),
            );
            y += 1;
        }
        if !intro.is_empty() {
            y += 1;
        }
        let avail = inner.bottom().saturating_sub(y + 2);
        let ph = props_h.min(avail);
        if ph > 0 {
            self.props
                .render(Rect::new(inner.x - 1, y, inner.width + 1, ph), buf, ctx, bg);
            y += ph;
        }
        if !self.detail.is_empty() {
            y += 1;
            let dh = inner.bottom().saturating_sub(y + 2);
            let wrapped: Vec<String> = self
                .detail
                .iter()
                .flat_map(|l| crate::ui::text::wrap(l, inner.width.saturating_sub(2) as usize))
                .collect();
            self.detail_scroll.set_content(wrapped.len());
            self.detail_scroll.set_viewport(dh as usize);
            for (k, i) in self.detail_scroll.visible_range().enumerate() {
                buf.set_string(inner.x, y + k as u16, &wrapped[i], t.secondary().bg(bg));
            }
            if self.detail_scroll.overflows() {
                let body = Rect::new(area.x, y, (area.right() - 1).saturating_sub(area.x), dh);
                self.detail_scroll
                    .draw_with_fade(body, buf, ctx, self.id, true, &[]);
            }
        }
        let ay = inner.bottom().saturating_sub(1);
        let mut widths: Vec<u16> = self.actions.iter().map(|b| b.width()).collect();
        widths.push(self.close.width());
        let rects = row_layout_right(Rect::new(inner.x, ay, inner.width, 1), &widths, 1);
        for (i, b) in self.actions.iter_mut().enumerate() {
            b.render(rects[i], buf, ctx, bg);
        }
        self.close.render(rects[self.actions.len()], buf, ctx, bg);
        for b in &self.actions {
            ctx.hits.register(b.id, b.area);
        }
        ctx.hits.register(self.close.id, self.close.area);
        for i in self.props.scroll.visible_range() {
            ctx.hits.raise(self.props.row_id(i));
        }
    }
}

impl Scrollable for InfoDialog {
    fn pane(&mut self) -> &mut ScrollPane {
        &mut self.detail_scroll
    }

    fn wheel_first(&mut self, axis: Axis, delta: i32) -> Option<Response<()>> {
        self.detail
            .is_empty()
            .then(|| self.props.wheel(axis, delta))
    }
}

impl InfoDialog {
    pub fn pointer_in(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
        focus: &mut Focus,
    ) -> Response<()> {
        use crate::core::id::Part;
        use crate::core::intent::Phase;
        use crate::core::pointer::MouseTarget;
        if let Some(mut r) = self.props.route(ev) {
            if ev.is(Phase::Click) && ev.index_of(Part::ROW).is_some() {
                focus.focus(self.props.id);
            }

            for action in r.take_actions() {
                if let crate::widgets::props::PropsAction::Activate(i) = action
                    && let Some(Some(v)) = self.copy_values.get(i)
                {
                    self.result = Some(InfoResult::Copy(v.clone()));
                }
            }
            return r.erase();
        }
        if ev.wheel().is_some() && !ev.on(self.id) {
            return Response::ignored();
        }
        if ev.on(self.id)
            && let Some(r) = crate::core::pointer::scroll_input(self, ev)
        {
            return r;
        }
        match ev.phase() {
            Some(Phase::Drag) => self.on_drag(ev.id, ev.pos()).into(),
            Some(Phase::Click) => self.on_click(ev.id, ev.pos(), focus).into(),
            _ => Response::ignored(),
        }
    }
}
